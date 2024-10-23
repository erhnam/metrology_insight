use crate::metrology_insight::signal_processing;

const FREQ_NOMINAL_50: f64 = 50.0;
const FREQ_NOMINAL_60: f64 = 60.0;

/* The ratio of the ADC to Voltage Values, used to scale samples to Volts. */
pub const ADC_VOLTAGE_D2A_FACTOR: f64 = 9289.14;
/* The ratio of the ADC to Current values, used to scale samples to Volts */
/* (factor from datasheet with values Vref+= 1.2, Vref-= 0, Gain= 1) */
pub const ADC_CURRENTS_D2A_FACTOR: f64 = 1048.5760;

const ADC_SAMPLES_SECOND: f64 =  7812.5;

const ADC_SAMPLES_50HZ_CYCLE: u32 = 157; /* round(ADC_SAMPLES_SECOND / 50)*/
const ADC_SAMPLES_60HZ_CYCLE: u32 = 131;

const FREQ_ZC_DEBOUNCE: u32 = 5;
const ZERO_CROSSING_MAX_POINTS: usize = 100; // Máximo de puntos de cruce por cero a almacenar

const EXTRA_SAMPLES: u32 = 20; /* Extra samples to a cycle to get zero crossing */

const NUMBER_HARMONICS: usize = 10;

/// Representa una señal de corriente o voltaje
#[derive(Default, Clone)]
pub struct MetrologyInsightSignal {
	pub signal: Vec<i32>,    // Buffer de la señal
	pub length: usize,          // Longitud del buffer de muestras (usualmente mayor a 1 ciclo)
	pub length_cycle: usize,    // Muestras en 1 ciclo de la señal (menor que la longitud del buffer)
	pub integrate: bool,     // Indica si la señal debe ser integrada
	pub calc_freq: bool,     // Indica si la frecuencia debe ser calculada desde la señal
	pub peak: f64,         // Valor pico de la señal
	pub rms: f64,          // Valor RMS de la señal
	pub freq_nominal: f64, // Frecuencia nominal (50Hz o 60Hz)
	pub freq_zc: f64,      // Frecuencia de la señal basada en el cruce por cero
	pub harmonics: [f64; NUMBER_HARMONICS], // Arreglo de amplitudes y fases de las armónicas
	pub thd: f64,            // Distorsión armónica total
	pub sc_thres: f64,     // Umbral de cortocircuito
}

fn is_frequency(freq: f64, nominal: f64) -> bool {
	freq < (1.07 * nominal) && freq > (0.95 * nominal)
}

pub fn calculate_zero_crossing_freq(signal: &[i32], length: usize) -> f64 {
	let mut num_crossing: usize = 0;
	let mut debounce: u32 = 0;
	let mut frequency: f64 = -1.0;
	let mut interpolation_points: Vec<f64> = vec![0.0; ZERO_CROSSING_MAX_POINTS];

	for i in 0..length - 1 {
		// Detectar un cruce por cero
		if (debounce == 0 && signal[i] > 0 && signal[i + 1] <= 0) || (signal[i] < 0 && signal[i + 1] >= 0) {
			// Interpolación para calcular el punto exacto de cruce
			let x1: f64 = i as f64;
			let y1: f64 = signal[i] as f64;
			let x2: f64 = (i + 1) as f64;
			let y2: f64 = signal[i + 1] as f64;

			// Interpolar el cruce por cero
			let yp: f64 = 0.0; // Valor en y en el cruce por cero
			let xp: f64 = x1 + (yp - y1) * ((x2 - x1) / (y2 - y1));

			// Almacenar el punto de interpolación
			if num_crossing < ZERO_CROSSING_MAX_POINTS {
				interpolation_points[num_crossing] = xp;
				num_crossing += 1; // Incrementar el contador de cruces
			}
			
			debounce = FREQ_ZC_DEBOUNCE; // Reiniciar el debounce
		}

		// Manejar el debounce
		if debounce > 0 {
			debounce -= 1;
		}
		
	}

	// Calcular la frecuencia a partir de los puntos de cruce
	if num_crossing > 1 {
		let mut sum: f64 = 0.0;
		for p in 0..num_crossing - 1 {
			sum += interpolation_points[p + 1] - interpolation_points[p];
		}
		let cycle_avg: f64 = (sum / (num_crossing - 1) as f64) * 2.0;
		frequency = 1.0 / (cycle_avg / ADC_SAMPLES_SECOND);
	}

	frequency
}

fn calculate_signal_frequency_nominal(freq_zc: f64, length: &mut usize, nominal_freq: f64) -> f64 {
	let mut freq_nominal: f64 = FREQ_NOMINAL_50;
	*length = ADC_SAMPLES_50HZ_CYCLE as usize;

	if is_frequency(freq_zc, FREQ_NOMINAL_60) {
		freq_nominal = FREQ_NOMINAL_60;
		if nominal_freq != FREQ_NOMINAL_60 {
			*length = ADC_SAMPLES_60HZ_CYCLE as usize;
		}
	}

	return freq_nominal;
}

fn signal_offset_remove(signal: &mut [i32]) {
	let max_val: i32 = *signal.iter().max().unwrap();
	let min_val: i32 = *signal.iter().min().unwrap();
	let offset: i32 = (max_val + min_val) / 2;

	for sample in signal.iter_mut() {
		*sample -= offset;
	}
}

fn limit_length_to_cycles(length: usize, frequency: f64) -> usize {
	let mut length_cycles: usize = 0;
	let one_cycle: usize = (ADC_SAMPLES_SECOND / frequency).round() as usize;

	while length_cycles + one_cycle <= length {
		length_cycles += one_cycle;
	}

	// La longitud de los ciclos no puede ser mayor que la longitud del buffer
	if length_cycles > length {
		length_cycles = length;
	}

	length_cycles
}

fn optimal_abs(value: i32) -> u32 {
	let temp: i32 = value >> 31;
	let toggled_value: i32 = value ^ temp;
	let abs_value: i32 = toggled_value.wrapping_add(temp & 1);

	abs_value as u32
}

fn short_circuit(signal: &[i32], length: usize) -> f64 {
	const ADC_SAMPLES_5_MS: usize = 10; // Ajustar según tus requisitos

	if length > ADC_SAMPLES_50HZ_CYCLE as usize {
		return 0.0;
	}

	// Convertir los valores absolutos en un vector
	let mut sorted_signal: Vec<u32> = signal.iter()
		.take(length)
		.map(|&s| optimal_abs(s))
		.collect();

	sorted_signal.sort();

	// Obtener el umbral
	let threshold_adc_counts = sorted_signal[ADC_SAMPLES_5_MS];

	threshold_adc_counts as f64
}

fn signal_integrate(signal: &mut [i32], length: usize, freq_zc: f64) {
	let mut integral: f64 = 0.0;
	let mut integrated_signal: Vec<i32> = Vec::new();
	let orms: f64 = signal_rms(signal, length, freq_zc) / signal_processing::ADC_CURRENTS_D2A_FACTOR;

	// Integración acumulativa por regla del trapecio
	for i in 0..signal.len() {
		let y_x: f64 = signal[i] as f64;
		let y_x1: f64 = if i + 1 < signal.len() { signal[i + 1] as f64 } else { y_x };

		integral += (y_x + y_x1) / 2.0; 
		integrated_signal.push(integral.round() as i32);
	}

	signal_offset_remove(&mut integrated_signal);

	// Escalar a 0 dB (atenuar frecuencias más altas): res_signal
	let integral_rms: f64 = signal_rms(&integrated_signal, length, freq_zc) / signal_processing::ADC_CURRENTS_D2A_FACTOR;

	let int_k: f64 = if orms != 0.0 { integral_rms / orms } else { 1.0 };

	for i in 0..integrated_signal.len() {
		integrated_signal[i] = (integrated_signal[i] as f64 / int_k).round() as i32;
	}

	// Modificar la señal original con la señal integrada sin offset
	for i in 0..length {
		signal[i] = integrated_signal[i] as i32;
	}

}

fn signal_peak(signal: &[i32], length: usize) -> f64 {
	let mut max_value = 0.0;

	for &value in &signal[0..length] {
		let abs_value = (value as f64).abs();
		if abs_value > max_value {
			max_value = abs_value;
		}
	}

	max_value
}

fn signal_rms(signal: &[i32], length_cycle: usize, frequency: f64) -> f64 {
	let mut square: f64 = 0.0;
	let mut n_length: f64 = 0.0; // Integer part
	let mut d_length: f64 = 0.0; // Decimal part
	let mut p_length: f64 = length_cycle as f64; // n + d length, fractional length of cycle
	let mut ysample: f64 = 0.0; // Last interpolated y sample at fractinal x

	if frequency > 0.0 {
		let cycle_length: f64 = ADC_SAMPLES_SECOND / frequency;
		n_length = cycle_length.floor();
		d_length = cycle_length.fract();
		p_length = n_length + d_length;
	}
	
    // Compute last interpolated sample
    if d_length > 0.0 {// Only interpolate frac sample if fractional part of cycle length exists.
        ysample = (((1.0-d_length)/2.0) * signal[n_length as usize - 1] as f64) + (((1.0 + d_length)/2.0) * signal[n_length as usize] as f64)
	}

    // Compute RMS integer N part
    for i in 0..n_length as u32 {
        let sample = signal[i as usize] as f64;
        square += sample.powi(2);
	}
	
	square += ysample.powi(2) * d_length;

	// Calculate mean
	let mean = square / p_length;

	if mean > 0.0 {
		mean.sqrt() as f64
	} else {
		0.0
	}
}

fn average(in_value: f64, out_value: &mut f64, avg: f64) {
	if *out_value == 0.0 { // Considerar como 0 si el valor de salida es 0
		*out_value = in_value; // Inicializar el valor de salida
	} else { // Si ya se ha inicializado el promedio
		let old_value = *out_value; // Guardar el viejo valor
		*out_value += avg * (in_value - old_value); // Actualizar el promedio
	}
}

// Aquí la función principal que procesa las señales
pub fn process_signal(signal: &mut MetrologyInsightSignal, calculated_adcfactor: f64) {
	let mut m_signal: MetrologyInsightSignal = MetrologyInsightSignal::default();

	if !signal.signal.is_empty() && signal.length > 0 {
		// Eliminar el offset de la señal
		signal_offset_remove(&mut signal.signal);

		// Se necesita calcular la frecuencia de cero cruce
		m_signal.freq_zc = calculate_zero_crossing_freq(&signal.signal, signal.length);
		if m_signal.freq_zc == -1.0 {
			m_signal.freq_zc = FREQ_NOMINAL_50; // Asignar frecuencia nominal en caso de error
		}

		signal.freq_zc = m_signal.freq_zc; // Indica la frecuencia calculada para esta señal
		signal.freq_nominal = calculate_signal_frequency_nominal(m_signal.freq_zc, &mut signal.length, signal.freq_nominal);
		signal.length_cycle = limit_length_to_cycles(signal.length, signal.freq_nominal);
		signal.length = signal.length_cycle + EXTRA_SAMPLES as usize;

		// TODO: Cálculos de armonías
		//harmonics(signal, calculated_adcfactor, signal.integrate, m_signal.freq_zc);

		if signal.integrate {
			signal_integrate(&mut signal.signal, signal.length_cycle, signal.freq_zc);
		}

		// Medición del corto circuito
		if signal.integrate {
			signal.sc_thres = short_circuit(&signal.signal, signal.length_cycle) / calculated_adcfactor;
		}

		// Cálculo del pico
		m_signal.peak = signal_peak(&signal.signal, signal.length_cycle) / calculated_adcfactor;

		// Cálculo del RMS
		m_signal.rms = signal_rms(&signal.signal, signal.length_cycle, m_signal.freq_zc) / calculated_adcfactor;

		// Asignar medidas a la señal (promediando)
		average(m_signal.rms, &mut signal.rms, 0.02);
		average(m_signal.freq_zc, &mut signal.freq_zc, 0.02);

		if m_signal.peak > signal.peak {
			signal.peak = m_signal.peak;
		}
	}
}