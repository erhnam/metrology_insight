use crate::metrology_insight::types::*;
use crate::metrology_insight::voltage_current::calculate_rms;

/*
* @brief Is the frequency within the tolerance range?
* @param freq Frequency to check
* @param nominal Nominal frequency
* @return true if the frequency is within the tolerance range, false otherwise
* @note This function checks if the frequency is within the tolerance range of 5% of the nominal frequency.
*/
fn is_frequency_in_tolerance(freq: f64, nominal: f64) -> bool {
    freq < (1.07 * nominal) && freq > (0.95 * nominal)
}

/*
* @brief Calculate the nominal frequency of a signal.
* @param freq_zc Frequency of the signal
* @param length Pointer to the length of the signal
* @param nominal_freq Nominal frequency
* @return Nominal frequency
* @note This function calculates the nominal frequency of a signal.

*/
fn calculate_nominal_frequency(freq_zc: f64, length: &mut usize, nominal_freq: f64) -> f64 {
    let mut freq_nominal = FREQ_NOMINAL_50;

    *length = ADC_SAMPLES_50HZ_CYCLE as usize;

    if is_frequency_in_tolerance(freq_zc, FREQ_NOMINAL_60) {
        freq_nominal = FREQ_NOMINAL_60;

        if nominal_freq != FREQ_NOMINAL_60 {
            *length = ADC_SAMPLES_60HZ_CYCLE;
        }
    }

    freq_nominal
}

/*
* @brief Calculate the zero crossing frequency of a signal.
* @param signal Pointer to the signal buffer
* @param adc_samples_second Number of ADC samples per second
* @return Frequency in Hz
* @note This function calculates the zero crossing frequency of a signal.
*/
fn calculate_zero_crossing_frequency(signal: &[f64], adc_samples_second: f64) -> f64 {
    let num_samples = signal.len();
    let mut num_crossing: usize = 0;
    let mut debounce: u32 = 0;
    let mut frequency: f64 = -1.0;
    let mut interpolation_points: Vec<f64> = vec![0.0; ZERO_CROSSING_MAX_POINTS];

    for p in 0..(num_samples - 1) {
        let y1: f64 = signal[p];
        let y2: f64 = signal[p + 1];

        // Cruce por cero sin necesidad de tolerancia (enteros, directamente)
        //if debounce == 0 && (y1 > 0 && y2 <= 0 || y1 < 0 && y2 >= 0) {
        if debounce == 0 && signal[p] < 0.0 && signal[p + 1] >= 0.0 {
            // Interpolación
            let x1 = p;
            let x2 = p + 1;

            let y1f = y1;
            let y2f = y2;

            // Interpolación lineal para cruce por cero
            if (y2f - y1f).abs() > f64::EPSILON {
                let xp = x1 as f64 + (0.0 - y1f) * (x2 - x1) as f64 / (y2f - y1f);

                if num_crossing < ZERO_CROSSING_MAX_POINTS {
                    interpolation_points[num_crossing] = xp;
                    num_crossing += 1;
                }
                debounce = FREQ_ZC_DEBOUNCE;
            }
        }

        if debounce > 0 {
            debounce -= 1;
        }
    }

    // Calcular frecuencia
    if num_crossing > 1 {
        let mut freq_sum = 0.0;
        let mut freq_count = 0;

        for p in 0..(num_crossing - 1) {
            let delta = interpolation_points[p + 1] - interpolation_points[p];
            if delta > 0.0 {
                freq_sum += 1.0 / (delta / adc_samples_second);
                freq_count += 1;
            }
        }

        if freq_count > 0 {
            //frequency = (freq_sum / freq_count as f64) / 2.0; // Corregimos doble cruce por ciclo
            frequency = freq_sum / freq_count as f64; // No dividimos por 2
        }
    }

    frequency
}

/*
* @brief Limit the length of a signal to a multiple of the cycle length.
* @param length Length of the signal
* @param frequency Frequency of the signal
* @param adc_samples_second Number of ADC samples per second
* @return Length of the signal limited to a multiple of the cycle length
* @note This function limits the length of a signal to a multiple of the cycle length.
*/
fn limit_length_to_cycles(length: usize, frequency: f64, adc_samples_second: f64) -> usize {
    let one_cycle: usize = (adc_samples_second / frequency).round() as usize;

    let length_cycles = (length / one_cycle) * one_cycle;

    length_cycles.min(length)
}

/*
* @brief Calculate the average of a signal.
* @param in_value Input value
* @param out_value Output value
* @param avg Average value
* @note This function calculates the average of a signal.
*/
pub fn update_average(in_value: f64, out_value: &mut f64, avg: f64) {
    if *out_value == 0.0 {
        *out_value = in_value;
    } else {
        let old_value = *out_value;
        *out_value += avg * (in_value - old_value);
    }
}

/*
* @brief Remove the offset from a signal.
* @param signal Pointer to the signal buffer
* @note This function removes the offset from a signal.
* @note The offset is calculated as the average of the maximum and minimum values of the signal.
*/
pub fn remove_signal_offset(signal: &mut [i32]) {
    let max = *signal.iter().max().unwrap();
    let min = *signal.iter().min().unwrap();
    let offset = (max + min) / 2;

    for s in signal.iter_mut() {
        *s -= offset;
    }
}

/*
* @brief Check if the signal is valid.
* @param signal Pointer to the signal buffer
* @param signal_type Type of the signal (voltage or current)
* @return true if the signal is valid, false otherwise
* @note This function checks if the signal is valid.
*/
fn is_signal_valid(signal: &[i32], signal_type: MetrologyInsightSignalType) -> bool {
    if signal.len() < 2 {
        return false;
    }

    let min_amplitude = signal_type.min_amplitude() as i32;

    let (min_val, max_val) = signal
        .iter()
        .fold((i32::MAX, i32::MIN), |(min, max), &x| (min.min(x), max.max(x)));

    let amplitude = max_val - min_val;

    amplitude >= min_amplitude
}

/*
* @brief Convert a raw signal to a real signal.
* @param signal Pointer to the signal buffer
* @return Real signal buffer
* @note This function converts a raw signal to a real signal.
*/
pub fn convert_raw_to_real_wave(signal: &MetrologyInsightSignal) -> Vec<f64> {
    let real_value: Vec<f64> = signal
        .wave
        .iter()
        .map(|&raw| {
            let voltaje_adc = raw as f64 * signal.adc_factor; // de RAW a voltios
            voltaje_adc * signal.adc_scale
        })
        .collect();

    real_value
}

/*
* @brief Process a signal.
* @param socket Pointer to the MetrologyInsightSocket structure.
* @param signal Pointer to the MetrologyInsightSignal structure.
* @param adc_samples_second Number of ADC samples per second.
* @param avg_sec Average time in seconds
* @note This function processes a signal.
*/
pub fn process_signal(
    socket: &mut MetrologyInsightSocket,
    signal: &mut MetrologyInsightSignal,
    adc_samples_second: f64,
    avg_sec: f64,
) {
    if is_signal_valid(&signal.wave, signal.signal_type) {
        remove_signal_offset(&mut signal.wave);

        // Remove offset from the signal
        let real_wave: Vec<f64> = convert_raw_to_real_wave(signal);

        // Convert to volts
        let freq_zc = if signal.calc_freq {
            let f = calculate_zero_crossing_frequency(&real_wave, adc_samples_second);
            if f == -1.0 {
                FREQ_NOMINAL_50
            } else {
                f
            }
        } else {
            socket.voltage_signal.freq_zc
        };

        // Calculate frequency
        signal.freq_zc = freq_zc;
        signal.freq_nominal = calculate_nominal_frequency(freq_zc, &mut signal.length, signal.freq_nominal);
        signal.length_cycle = limit_length_to_cycles(signal.length, signal.freq_nominal, adc_samples_second);
        signal.length = signal.length_cycle + EXTRA_SAMPLES;

        // Calculate Peak
        let peak = real_wave.iter().copied().fold(f64::MIN, f64::max);
        if peak > signal.peak {
            signal.peak = peak;
        }

        // Calculate RMS
        let rms = calculate_rms(&real_wave, signal.length_cycle, signal.freq_zc, adc_samples_second);

        // Asign values to signal
        match signal.signal_type {
            MetrologyInsightSignalType::Voltage => {
                socket.voltage_signal.real_wave = real_wave;
                socket.voltage_signal.freq_nominal = signal.freq_nominal;
                socket.voltage_signal.length_cycle = signal.length_cycle;
                socket.voltage_signal.length = signal.length;
                socket.voltage_signal.peak = signal.peak;
                update_average(rms, &mut socket.voltage_signal.rms, avg_sec);
                update_average(freq_zc, &mut socket.voltage_signal.freq_zc, avg_sec);
            }
            MetrologyInsightSignalType::Current => {
                socket.current_signal.real_wave = real_wave;
                socket.current_signal.freq_nominal = signal.freq_nominal;
                socket.current_signal.length_cycle = signal.length_cycle;
                socket.current_signal.length = signal.length;
                socket.current_signal.sc_thres = signal.sc_thres;
                socket.current_signal.peak = signal.peak;
                socket.current_signal.freq_zc = freq_zc;
                update_average(rms, &mut socket.current_signal.rms, avg_sec);
                update_average(freq_zc, &mut socket.current_signal.freq_zc, avg_sec);
            }
        }
    }
}
