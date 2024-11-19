use super::power;

pub fn calculate_peak(signal: &[i32], length: usize) -> f64 {
	let mut max_value = 0.0;

	for &value in &signal[0..length] {
		let abs_value = (value as f64).abs();
		if abs_value > max_value {
			max_value = abs_value;
		}
	}

	max_value
}

fn calculate_signal_power(signal1: &[i32],signal2: &[i32], length: usize, frequency: f64, adc_samples_second: f64) -> f64 {
    let mut square: f64 = 0.0;

    // Partes entera y fraccionaria
    let n_length = ((length - 1) as usize) as f64;
    let mut d_length = 0.0;
    let mut p_length = length as f64;

    // Calcular p_length si la frecuencia es mayor a 0
    if frequency > 0.0 {
        d_length = (adc_samples_second / frequency).fract();
        p_length = n_length + d_length;
    }

    // Calcular RMS para la parte entera
    for i in 0..(n_length as usize -1) {
        let sample1 = signal1[i] as f64;
        let sample2 = signal2[i] as f64;
        square += sample1 * sample2;
    }

    // Calcular muestra interpolada si existe parte fraccionaria
    if d_length != 0.0 {
        let last_index = n_length as usize - 1;
        let ysample1 = signal1[last_index] as f64
            + (signal1[last_index + 1] as f64 - signal1[last_index] as f64) * d_length;
        let ysample2 = signal2[last_index] as f64
            + (signal2[last_index + 1] as f64 - signal2[last_index] as f64) * d_length;
        square += (ysample1 * ysample2) * d_length;
    }

    // Calcular el valor medio
    square / p_length
}

pub fn calculate_rms(signal: &[i32], length_cycle: usize, frequency: f64, adc_samples_second: f64) -> f64 {
	let power: f64 = calculate_signal_power(signal, signal, length_cycle, frequency, adc_samples_second);
	if power > 0.0 {
		power.sqrt()
	} else {
		0.0
	}
}

pub fn calculate_phase_angle_from_signal_values(signal1: &[i32], signal2: &[i32], freq_zc: f64, length: usize, adc_samples_second: f64) -> f64 {
    //Calculate derived values
    let rms1: f64 = calculate_rms(signal1, length, freq_zc, adc_samples_second);
    let rms2: f64 = calculate_rms(signal2, length, freq_zc, adc_samples_second);
    let apparent_power: f64 = rms1 * rms2;
    let real_power: f64 = power::calculate_real_power_from_signals(signal1, signal2, length);
    let react_power: f64 = power::calculate_react_power_from_signals(signal1, signal2, length);

    power::calculate_phase_angle_from_power_values(apparent_power, real_power, react_power)
}