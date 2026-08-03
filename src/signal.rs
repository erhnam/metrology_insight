//! Per-cycle signal processing: RMS, PLL update, resampling and quality flags.
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    FftCache, MetrologyInsightSignal, MetrologyInsightSignalType,
    ADC_SAMPLES_50HZ_CYCLE, ADC_SAMPLES_60HZ_CYCLE, FFT_RESOLUTION, FREQ_NOMINAL_50,
    FREQ_NOMINAL_60, CYCLES_PER_WINDOW,
    Q_FLAG_OK, Q_FLAG_PLL_UNSETTLED, Q_FLAG_SYNC_INCONSISTENT
};
use crate::voltage_current::calculate_rms;
use super::pll::update_pll;
use super::resampling::resample_synchronous_into;
use crate::pll::PLL_ERROR_ACCUM_THRESHOLD;

pub const ZERO_CROSSING_MAX_POINTS: usize = 3;
pub const FREQ_ZC_DEBOUNCE: u32 = 2;

pub const EXTRA_SAMPLES: usize = 0;

// Frequency tolerance band: signal is valid if within [FREQ_TOLERANCE_LOW, FREQ_TOLERANCE_HIGH] * nominal
pub const FREQ_TOLERANCE_HIGH: f32 = 1.07;
pub const FREQ_TOLERANCE_LOW: f32 = 0.95;
// Minimum half-cycle fraction required (40% of one cycle) to trigger a valid half-cycle RMS calculation
pub const HALF_CYCLE_MIN_FACTOR: f32 = 0.4;
// Minimum RMS before computing the synchronous-resampled consistency error
pub const RMS_CONSISTENCY_MIN_GUARD: f32 = 1e-6;
// RMS vs sync-RMS relative error above which Q_FLAG_SYNC_INCONSISTENT is raised
pub const SYNC_CONSISTENCY_THRESHOLD: f32 = 0.001;

/// Checks whether the measured frequency lies within the tolerance band around `nominal`.
///
/// # Arguments
///
/// * `freq` - Measured frequency in Hz.
/// * `nominal` - Nominal frequency in Hz.
///
/// # Returns
///
/// `true` if `freq` is within `[FREQ_TOLERANCE_LOW, FREQ_TOLERANCE_HIGH] * nominal`.
fn is_frequency_in_tolerance(freq: f32, nominal: f32) -> bool {
    freq < (FREQ_TOLERANCE_HIGH * nominal) && freq > (FREQ_TOLERANCE_LOW * nominal)
}

/// Determines the nominal grid frequency (50 or 60 Hz) and the matching cycle length.
///
/// # Arguments
///
/// * `freq_zc` - Zero-crossing measured frequency in Hz.
/// * `length` - Cycle length in samples; updated to match the detected nominal frequency.
/// * `nominal_freq` - Previously assumed nominal frequency.
///
/// # Returns
///
/// The detected nominal frequency in Hz.
fn calculate_nominal_frequency(freq_zc: f32, length: &mut usize, nominal_freq: f32) -> f32 {
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

/// Estimates the grid frequency from interpolated rising zero crossings.
///
/// # Arguments
///
/// * `signal` - Input signal samples.
/// * `adc_samples_second` - ADC sampling rate in samples per second.
///
/// # Returns
///
/// The estimated frequency in Hz, or -1.0 if fewer than two crossings were found.
fn calculate_zero_crossing_frequency(signal: &[f32], adc_samples_second: f32) -> f32 {
    let num_samples = signal.len();
    let mut num_crossing: usize = 0;
    let mut debounce: u32 = 0;
    let mut frequency: f32 = -1.0;
    let mut interpolation_points = [0.0f32; ZERO_CROSSING_MAX_POINTS];

    for p in 0..(num_samples - 1) {
        let y1: f32 = signal[p];
        let y2: f32 = signal[p + 1];

        if debounce == 0 && signal[p] < 0.0 && signal[p + 1] >= 0.0 {
            let x1 = p;
            let x2 = p + 1;

            let y1f = y1;
            let y2f = y2;

            if (y2f - y1f).abs() > f32::EPSILON {
                let xp = x1 as f32 + (0.0 - y1f) * (x2 - x1) as f32 / (y2f - y1f);

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
            frequency = freq_sum / freq_count as f32;
        }
    }

    frequency
}

/// Rounds `length` down to a whole number of cycles at the given frequency.
///
/// # Arguments
///
/// * `length` - Desired window length in samples.
/// * `frequency` - Signal frequency in Hz.
/// * `adc_samples_second` - ADC sampling rate in samples per second.
///
/// # Returns
///
/// The largest multiple of one cycle that does not exceed `length`.
fn limit_length_to_cycles(length: usize, frequency: f32, adc_samples_second: f32) -> usize {
    let one_cycle: usize = (adc_samples_second / frequency).round() as usize;

    let length_cycles = (length / one_cycle) * one_cycle;

    length_cycles.min(length)
}

/// Updates an exponential moving average with a new input value.
///
/// # Arguments
///
/// * `in_value` - New measurement to fold into the average.
/// * `out_value` - Average being updated in place; seeded with `in_value` on the first call.
/// * `avg` - Smoothing factor applied to the difference between the new and old values.
pub fn update_average(in_value: f32, out_value: &mut f32, avg: f32) {
    if *out_value == 0.0 {
        *out_value = in_value;
    } else {
        let old_value = *out_value;
        *out_value += avg * (in_value - old_value);
    }
}

/// Removes the DC component by subtracting the signal mean from every sample.
///
/// # Arguments
///
/// * `signal` - Signal samples, modified in place.
pub fn remove_signal_offset(signal: &mut [f32]) {
    if signal.is_empty() { return; }
    let sum: f32 = signal.iter().sum();
    let offset = sum / signal.len() as f32;

    for s in signal.iter_mut() {
        *s -= offset;
    }
}

/// Checks whether the signal's peak-to-peak amplitude meets the minimum for its type.
///
/// # Arguments
///
/// * `signal` - Signal samples.
/// * `signal_type` - Signal type used to look up the minimum amplitude.
/// * `config` - Configuration providing the minimum amplitude for `signal_type`.
///
/// # Returns
///
/// `true` if the signal has at least 2 samples and sufficient amplitude.
fn is_signal_valid(signal: &[f32], signal_type: MetrologyInsightSignalType, config: &MetrologyInsightConfig) -> bool {
    if signal.len() < 2 {
        return false;
    }

    let min_amplitude = signal_type.min_amplitude(config);

    let (min_val, max_val) = signal
        .iter()
        .fold((f32::MAX, f32::MIN), |(min, max), &x| (min.min(x), max.max(x)));

    let amplitude = max_val - min_val;

    amplitude >= min_amplitude
}

#[cfg(feature = "alloc")]
/// Integrates the signal and normalizes the result so its RMS equals the input RMS.
///
/// # Arguments
///
/// * `s` - Input signal samples.
/// * `frequency_zc` - Zero-crossing frequency in Hz.
/// * `adc_samples_second` - ADC sampling rate in samples per second.
///
/// # Returns
///
/// A vector with the integral waveform scaled to match the input RMS.
pub fn signal_integrate(s: &[f32], frequency_zc: f32, adc_samples_second: f32) -> alloc::vec::Vec<f32> {
    let mut integral: f32 = 0.0;
    let mut res_signal: alloc::vec::Vec<f32> = alloc::vec::Vec::with_capacity(s.len());

    let orms = calculate_rms(s, s.len(), frequency_zc, adc_samples_second);

    for i in 0..s.len() {
        let y_x = s[i];
        let y_x1 = if i + 1 < s.len() { s[i + 1] } else { y_x };
        integral += (y_x + y_x1) / 2.0;
        res_signal.push(integral);
    }

    let integral_rms = calculate_rms(&res_signal, s.len(), frequency_zc, adc_samples_second);

    let int_k = if orms != 0.0 { integral_rms / orms } else { 1.0 };

    for i in 0..res_signal.len() {
        res_signal[i] = res_signal[i] / int_k;
    }

    res_signal
}

use crate::types::MetrologyInsightConfig;

/// Processes one metrology signal: offset removal, frequency detection, RMS, PLL, harmonics and quality flags.
///
/// # Arguments
///
/// * `signal` - Signal state, updated in place.
/// * `reference_freq_zc` - Reference zero-crossing frequency used for current channels or when `calc_freq` is false.
/// * `phase_delay_us` - Phase delay in microseconds applied during synchronous resampling.
/// * `config` - Metrology configuration.
/// * `fft_cache` - Shared FFT and sync buffer cache.
pub fn process_signal(
    signal: &mut MetrologyInsightSignal,
    reference_freq_zc: f32,
    phase_delay_us: f32,
    config: &MetrologyInsightConfig,
    fft_cache: &mut FftCache,
) {
    let adc_samples_second = config.adc_samples_seconds;
    let in_len = signal.real_wave_len;

    if is_signal_valid(&signal.real_wave[..in_len], signal.signal_type, config) {
        remove_signal_offset(&mut signal.real_wave[..in_len]);

        let real_slice = &signal.real_wave[..signal.real_wave_len];

        let freq_zc = if signal.calc_freq {
            let f = calculate_zero_crossing_frequency(real_slice, adc_samples_second);
            if f == -1.0 { config.nominal_freq } else { f }
        } else {
            reference_freq_zc
        };

        signal.freq_zc = freq_zc;
        signal.freq_nominal = calculate_nominal_frequency(freq_zc, &mut signal.length, signal.freq_nominal);
        signal.length_cycle = limit_length_to_cycles(signal.length, signal.freq_nominal, adc_samples_second);
        signal.length = signal.length_cycle + EXTRA_SAMPLES;

        let min_samples_half_cycle = signal.length_cycle as f32 * HALF_CYCLE_MIN_FACTOR;
        let mut prev_v = if real_slice.is_empty() { 0.0 } else { real_slice[0] };

        for &v in real_slice.iter() {
            signal.urms_half_cycle.process_sample(v);
            if (prev_v < 0.0 && v >= 0.0) || (prev_v >= 0.0 && v < 0.0) {
                signal.urms_half_cycle.half_cycle_trigger(min_samples_half_cycle);
            }
            prev_v = v;
        }

        let peak = real_slice.iter().copied().fold(f32::MIN, f32::max);
        if peak > signal.peak {
            signal.peak = peak;
        }

        let rms = calculate_rms(real_slice, signal.length_cycle, signal.freq_zc, adc_samples_second);

        if !signal.is_current() {
            update_pll(&mut signal.pll_state, real_slice, adc_samples_second, config.nominal_freq, &config.pll);
        }

        let freq_ref = if signal.is_current() { reference_freq_zc } else { signal.pll_state.freq_est };

        let sync_len = resample_synchronous_into(
            real_slice,
            adc_samples_second,
            freq_ref,
            CYCLES_PER_WINDOW,
            FFT_RESOLUTION,
            phase_delay_us,
            fft_cache.sync_buffer.as_mut(),
        );

        if sync_len > 0 {
            let sync_slice = &fft_cache.sync_buffer[..sync_len];
            let mut sum_sq = 0.0;
            for &val in sync_slice.iter() {
                sum_sq += val * val;
            }
            let rms_sync = f32::sqrt(sum_sq / (sync_len as f32));
            signal.rms_sync = rms_sync;

            if signal.rms > RMS_CONSISTENCY_MIN_GUARD {
                signal.consistency_error = (signal.rms - signal.rms_sync).abs() / signal.rms;
            }

            let mut flags = Q_FLAG_OK;
            if !signal.pll_state.locked || signal.pll_state.error_accum > PLL_ERROR_ACCUM_THRESHOLD {
                flags |= Q_FLAG_PLL_UNSETTLED;
            }
            if signal.consistency_error > SYNC_CONSISTENCY_THRESHOLD {
                flags |= Q_FLAG_SYNC_INCONSISTENT;
            }
            signal.quality_flags = flags;
        }
        
        if sync_len >= FFT_RESOLUTION {
            if let Some((harmonics, thd)) = fft_cache.compute_from_sync_buffer() {
                for i in 0..harmonics.len() {
                    update_average(harmonics[i], &mut signal.harmonics[i], config.avg_sec);
                }
                update_average(thd, &mut signal.thd, config.avg_sec);
            }
        }

        update_average(rms, &mut signal.rms, config.avg_sec);
        update_average(peak, &mut signal.peak, config.avg_sec);

        // 10-cycle RMS accumulation (EN 61000-4-30 Sec 5.2 requirement)
        signal.cycle_10_sq_sum += rms * rms;
        signal.cycle_10_count += 1;
        if signal.cycle_10_count >= 10 {
            signal.rms_10cycle = (signal.cycle_10_sq_sum / 10.0).sqrt();
            signal.cycle_10_sq_sum = 0.0;
            signal.cycle_10_count = 0;
        }
    }
}