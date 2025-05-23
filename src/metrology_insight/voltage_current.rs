/*
*@brief Calculate the signal power
* @param signal1 Pointer to the first signal array.
* @param signal2 Pointer to the second signal array.
* @param length Length of the signals.
* @param frequency Frequency of the signals.
* @param adc_samples_second ADC samples per second.
* @param data Pointer to the MetrologyInsightSocket structure.
*/
fn calculate_signal_power(
    signal1: &[f64],
    signal2: &[f64],
    length: usize,
    frequency: f64,
    adc_samples_second: f64,
) -> f64 {
    if length == 0 || signal1.is_empty() || signal2.is_empty() {
        return 0.0;
    }

    let n_length = (length - 1) as f64;
    let mut d_length = 0.0;
    let mut p_length = length as f64;

    if frequency > 0.0 {
        d_length = (adc_samples_second / frequency).fract();
        p_length = n_length + d_length;
    }

    let n_length_usize = n_length as usize;

    let mut square: f64 = 0.0;

    // Bucle hasta length - 1 para incluir todas las muestras enteras
    for i in 0..n_length_usize {
        if i >= signal1.len() || i >= signal2.len() {
            break;
        }
        let sample1 = signal1[i] as f64;
        let sample2 = signal2[i] as f64;
        square += sample1 * sample2;
    }
    // Interpolación para la parte fraccional
    if d_length != 0.0 && n_length_usize + 1 < signal1.len() && n_length_usize + 1 < signal2.len() {
        if n_length_usize + 1 >= signal1.len() || n_length_usize + 1 >= signal2.len() {
            log::info!("Error: signal length is too short for interpolation.");
            return 0.0;
        }
        let ysample1 = signal1[n_length_usize] as f64
            + (signal1[n_length_usize + 1] as f64 - signal1[n_length_usize] as f64) * d_length;
        let ysample2 = signal2[n_length_usize] as f64
            + (signal2[n_length_usize + 1] as f64 - signal2[n_length_usize] as f64) * d_length;
        square += (ysample1 * ysample2) * d_length;
    }

    square / p_length
}

/*
*@brief Calculate the RMS value of a signal
* @param signal Pointer to the signal array.
* @param length_cycle Length of the cycle.
* @param frequency Frequency of the signal.
* @param adc_samples_second ADC samples per second.
* @return The RMS value of the signal.
*/
pub fn calculate_rms(signal: &[f64], length_cycle: usize, frequency: f64, adc_samples_second: f64) -> f64 {
    if length_cycle == 0 || signal.is_empty() {
        return 0.0;
    }

    let power: f64 = calculate_signal_power(signal, signal, length_cycle, frequency, adc_samples_second);

    if power > 0.0 {
        power.sqrt()
    } else {
        0.0
    }
}
