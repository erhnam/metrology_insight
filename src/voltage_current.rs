//! Voltage and current RMS calculation and sample integration.
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

/// Compute the average instantaneous product of two signals over one cycle.
///
/// Uses fractional-cycle interpolation when `frequency` is non-zero so that a
/// non-integer number of samples per cycle is handled correctly.
///
/// # Arguments
///
/// * `signal1` — First signal samples.
/// * `signal2` — Second signal samples.
/// * `length` — Number of samples to integrate over.
/// * `frequency` — Fundamental frequency in Hz (0 disables interpolation).
/// * `adc_samples_second` — ADC sampling rate in samples per second.
///
/// # Returns
///
/// The averaged product (e.g. mean power), or 0.0 when the input is invalid or
/// the signals are too short for interpolation.
fn calculate_signal_power(signal1: &[f32], signal2: &[f32], length: usize, frequency: f32, adc_samples_second: f32) -> f32 {
    if length == 0 || signal1.is_empty() || signal2.is_empty() {
        return 0.0;
    }

    let n_length = (length - 1) as f32;
    let mut d_length = 0.0;
    let mut p_length = length as f32;

    if frequency > 0.0 {
        d_length = (adc_samples_second / frequency).fract();
        p_length = n_length + d_length;
    }

    let n_length_usize = n_length as usize;

    let mut square: f32 = 0.0;

    for i in 0..n_length_usize {
        if i >= signal1.len() || i >= signal2.len() {
            break;
        }
        let sample1 = signal1[i];
        let sample2 = signal2[i];
        square += sample1 * sample2;
    }
    if d_length != 0.0 && n_length_usize + 1 < signal1.len() && n_length_usize + 1 < signal2.len() {
        if n_length_usize + 1 >= signal1.len() || n_length_usize + 1 >= signal2.len() {
            log::info!("Error: signal length is too short for interpolation.");
            return 0.0;
        }
        let ysample1 = signal1[n_length_usize]
            + (signal1[n_length_usize + 1] - signal1[n_length_usize]) * d_length;
        let ysample2 = signal2[n_length_usize]
            + (signal2[n_length_usize + 1] - signal2[n_length_usize]) * d_length;
        square += (ysample1 * ysample2) * d_length;
    }

    square / p_length
}

/// Calculate the RMS value of a signal over one cycle.
///
/// # Arguments
///
/// * `signal` — Signal samples.
/// * `length_cycle` — Number of samples in one cycle.
/// * `frequency` — Fundamental frequency in Hz (0 disables interpolation).
/// * `adc_samples_second` — ADC sampling rate in samples per second.
///
/// # Returns
///
/// The RMS value, or 0.0 when the input is empty or the computed power is zero.
pub fn calculate_rms(signal: &[f32], length_cycle: usize, frequency: f32, adc_samples_second: f32) -> f32 {
    if length_cycle == 0 || signal.is_empty() {
        return 0.0;
    }

    let power: f32 = calculate_signal_power(signal, signal, length_cycle, frequency, adc_samples_second);

    if power > 0.0 {
        power.sqrt()
    } else {
        0.0
    }
}
