use crate::metrology_insight::signal_processing;
use crate::metrology_insight::voltage_current;

const FREQ_NOMINAL_50: f64 = 50.0;
const FREQ_NOMINAL_60: f64 = 60.0;

pub const AVG_SEC: f64 = 0.02;

/* The ratio of the ADC to Voltage Values, used to scale samples to Volts. */
pub const ADC_VOLTAGE_D2A_FACTOR: f64 = 9289.14;
/* The ratio of the ADC to Current values, used to scale samples to Volts */
/* (factor from datasheet with values Vref+= 1.2, Vref-= 0, Gain= 1) */
pub const ADC_CURRENTS_D2A_FACTOR: f64 = 1048.5760;

pub const ADC_SAMPLES_SECOND: f64 =  7812.5;

const ADC_SAMPLES_50HZ_CYCLE: u32 = 157; /* round(ADC_SAMPLES_SECOND / 50)*/
const ADC_SAMPLES_60HZ_CYCLE: u32 = 131;

const FREQ_ZC_DEBOUNCE: u32 = 5;
const ZERO_CROSSING_MAX_POINTS: usize = 100; // Maximum number of zero crossing points to store

const EXTRA_SAMPLES: u32 = 20; /* Extra samples to a cycle to get zero crossing */

const NUMBER_HARMONICS: usize = 10;

/// Represents a current or voltage signal.
#[derive(Default, Clone)]
pub struct MetrologyInsightSignal {
    pub signal: Vec<i32>,    // Signal buffer
    pub length: usize,          // Length of the sample buffer (usually greater than 1 cycle)
    pub length_cycle: usize,    // Samples in 1 cycle of the signal (less than the buffer length)
    pub integrate: bool,     // Indicates if the signal should be integrated
    pub calc_freq: bool,     // Indicates if the frequency should be calculated from the signal
    pub peak: f64,         // Peak value of the signal
    pub rms: f64,          // RMS value of the signal
    pub freq_nominal: f64, // Nominal frequency (50Hz or 60Hz)
    pub freq_zc: f64,      // Frequency of the signal based on zero crossing
    pub harmonics: [f64; NUMBER_HARMONICS], // Array of amplitudes and phases of harmonics
    pub thd: f64,            // Total harmonic distortion
    pub sc_thres: f64,     // Short circuit threshold
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
        // Detect a zero crossing
        if (debounce == 0 && signal[i] > 0 && signal[i + 1] <= 0) || (signal[i] < 0 && signal[i + 1] >= 0) {
            // Interpolation to calculate the exact crossing point
            let x1: f64 = i as f64;
            let y1: f64 = signal[i] as f64;
            let x2: f64 = (i + 1) as f64;
            let y2: f64 = signal[i + 1] as f64;

            // Interpolate the zero crossing
            let yp: f64 = 0.0; // Value in y at the zero crossing
            let xp: f64 = x1 + (yp - y1) * ((x2 - x1) / (y2 - y1));

            // Store the interpolation point
            if num_crossing < ZERO_CROSSING_MAX_POINTS {
                interpolation_points[num_crossing] = xp;
                num_crossing += 1; // Increment the crossing counter
            }

            debounce = FREQ_ZC_DEBOUNCE; // Reset the debounce
        }

        // Handle the debounce
        if debounce > 0 {
            debounce -= 1;
        }
    }

    // Calculate the frequency from the crossing points
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
	const ADC_SAMPLES_5_MS: usize = 10;

	if length > ADC_SAMPLES_50HZ_CYCLE as usize {
		return 0.0;
	}

	let mut sorted_signal: Vec<u32> = signal.iter()
		.take(length)
		.map(|&s| optimal_abs(s))
		.collect();

	sorted_signal.sort();

	let threshold_adc_counts: u32 = sorted_signal[ADC_SAMPLES_5_MS];

	threshold_adc_counts as f64
}

fn signal_integrate(signal: &mut [i32], length: usize, freq_zc: f64) {
	let mut integral: f64 = 0.0;
	let mut integrated_signal: Vec<i32> = Vec::new();
	let orms: f64 = voltage_current::calculate_rms(signal, length, freq_zc) / signal_processing::ADC_CURRENTS_D2A_FACTOR;

	// Cumulative integration by trapezoid rule
	for i in 0..signal.len() {
		let y_x: f64 = signal[i] as f64;
		let y_x1: f64 = if i + 1 < signal.len() { signal[i + 1] as f64 } else { y_x };

		integral += (y_x + y_x1) / 2.0; 
		integrated_signal.push(integral.round() as i32);
	}

	signal_offset_remove(&mut integrated_signal);

	// Scale to 0 dB (attenuate higher frequencies): res_signal
	let integral_rms: f64 = voltage_current::calculate_rms(&integrated_signal, length, freq_zc) / signal_processing::ADC_CURRENTS_D2A_FACTOR;

	let int_k: f64 = if orms != 0.0 { integral_rms / orms } else { 1.0 };

	for i in 0..integrated_signal.len() {
		integrated_signal[i] = (integrated_signal[i] as f64 / int_k).round() as i32;
	}

	// Modifying the original signal with the integrated signal without offset
	for i in 0..length {
		signal[i] = integrated_signal[i] as i32;
	}

}

pub fn average(in_value: f64, out_value: &mut f64, avg: f64) {
	if *out_value == 0.0 {
		*out_value = in_value;
	} else {
		let old_value = *out_value;
		*out_value += avg * (in_value - old_value);
	}
}
pub fn process_signal(signal: &mut MetrologyInsightSignal, calculated_adcfactor: f64) {
    let mut m_signal: MetrologyInsightSignal = MetrologyInsightSignal::default();

    if !signal.signal.is_empty() && signal.length > 0 {
        // Remove the offset from the signal
        signal_offset_remove(&mut signal.signal);

        // Zero crossing frequency needs to be calculated
        m_signal.freq_zc = calculate_zero_crossing_freq(&signal.signal, signal.length);
        if m_signal.freq_zc == -1.0 {
            m_signal.freq_zc = FREQ_NOMINAL_50; // Assign nominal frequency in case of error
        }

        signal.freq_zc = m_signal.freq_zc; // Indicates the calculated frequency for this signal
        signal.freq_nominal = calculate_signal_frequency_nominal(m_signal.freq_zc, &mut signal.length, signal.freq_nominal);
        signal.length_cycle = limit_length_to_cycles(signal.length, signal.freq_nominal);
        signal.length = signal.length_cycle + EXTRA_SAMPLES as usize;

        // TODO: Harmonics calculations
        // harmonics(signal, calculated_adcfactor, signal.integrate, m_signal.freq_zc);

        if signal.integrate {
            signal_integrate(&mut signal.signal, signal.length_cycle, signal.freq_zc);
        }

        // Short circuit measurement
        if signal.integrate {
            signal.sc_thres = short_circuit(&signal.signal, signal.length_cycle) / calculated_adcfactor;
        }

        // Peak calculation
        m_signal.peak = voltage_current::calculate_peak(&signal.signal, signal.length_cycle) / calculated_adcfactor;

        // RMS calculation
        m_signal.rms = voltage_current::calculate_rms(&signal.signal, signal.length_cycle, m_signal.freq_zc) / calculated_adcfactor;

        // Assign measurements to the signal (averaging)
        average(m_signal.rms, &mut signal.rms, AVG_SEC);
        average(m_signal.freq_zc, &mut signal.freq_zc, AVG_SEC);

        if m_signal.peak > signal.peak {
            signal.peak = m_signal.peak;
        }
    }
}
