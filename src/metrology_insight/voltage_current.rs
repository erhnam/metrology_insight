use super::signal_processing::ADC_SAMPLES_SECOND;
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

pub fn calculate_rms(signal: &[i32], length_cycle: usize, frequency: f64) -> f64 {
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

pub fn calculate_phase_angle_from_signal_values(signal1: &[i32], signal2: &[i32], freq_zc: f64, length: usize) -> f64 {
    //Calculate derived values
    let rms1: f64 = calculate_rms(signal1, length, freq_zc);
    let rms2: f64 = calculate_rms(signal2, length, freq_zc);
    let apparent_power: f64 = rms1 * rms2;
    let real_power: f64 = power::calculate_real_power_from_signals(signal1, signal2, length);
    let react_power: f64 = power::calculate_react_power_from_signals(signal1, signal2, length);

    power::calculate_phase_angle_from_power_values(apparent_power, real_power, react_power)
}