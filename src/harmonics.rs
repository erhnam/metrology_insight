//! Harmonic and interharmonic spectral analysis via FFT.
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

#![allow(dead_code)]

use alloc::boxed::Box;

pub const FFT_RESOLUTION: usize = 512;
pub const CYCLES_PER_WINDOW: usize = 1;
pub const FFT_MIN_FUNDAMENTAL_MAG: f32 = 1e-4;
// No longer used directly: compute_magnitudes now derives its own search
// window from CYCLES_PER_WINDOW. Kept for reference / backward compatibility.
pub const FFT_FUND_SEARCH_BINS: usize = 3;

/// Number of interharmonic subgroup bands between H1 and H50.
pub const INTERHARMONIC_GROUPS: usize = 49;
/// Cycles to accumulate for interharmonic subgroup computation.
pub const CYCLES_FOR_INTERHARMONIC: usize = 10;

const MAGNITUDES_LEN: usize = FFT_RESOLUTION / 2 + 1; // 257

// Size of the raw ring buffer that stores samples at nominal FS.
// Must be large enough to cover CYCLES_PER_WINDOW cycles even if the
// real frequency drops below nominal (e.g. 45 Hz during grid sags).
// With FS=8000Hz and an expected minimum freq of ~45Hz: 8000/45 ≈ 178 samples/cycle.
// We give a x2 margin for safety.
const RAW_BUFFER_LEN: usize = 512;

/// Resamples a signal from its original length to a new length via linear interpolation.
///
/// # Arguments
///
/// * `signal` - Input time-domain samples.
/// * `new_len` - Desired output length.
///
/// # Returns
///
/// A new vector of `new_len` linearly interpolated samples.
pub fn resample_signal(signal: &[f32], new_len: usize) -> alloc::vec::Vec<f32> {
    let n = signal.len();
    let step = n as f32 / new_len as f32;
    let mut resampled = alloc::vec::Vec::with_capacity(new_len);

    for i in 0..new_len {
        let pos = i as f32 * step;
        let idx0 = crate::math::floor(pos) as usize % n;
        let idx1 = (idx0 + 1) % n;
        let fraction = pos - idx0 as f32;

        let y0 = signal[idx0];
        let y1 = signal[idx1];
        resampled.push(y0 + (y1 - y0) * fraction);
    }
    resampled
}

/// Calculates harmonic percentages normalized to H1 (100%) and the Total Harmonic Distortion (THD %).
///
/// # Arguments
/// * `magnitudes` - Slice containing absolute frequency magnitudes.
/// * `fund_bin` - Index of the fundamental frequency bin (H1).
/// * `fundamental_mag` - Absolute magnitude of the fundamental frequency.
///
/// # Returns
/// * A tuple containing an array of harmonic percentages (`[H1, H2, ..., Hn]`) and the THD percentage.
fn calculate_harmonics_and_thd(
    magnitudes: &[f32],
    fund_bin: usize,
    fundamental_mag: f32,
) -> ([f32; crate::types::NUMBER_HARMONICS], f32) {
    let mut harmonics = [0.0; crate::types::NUMBER_HARMONICS];
    let mut thd_sq_sum = 0.0;

    harmonics[0] = 100.0; // H1 is 100%

    if fundamental_mag < FFT_MIN_FUNDAMENTAL_MAG || fund_bin == 0 {
        return (harmonics, 0.0);
    }

    for order in 2..=crate::types::NUMBER_HARMONICS {
        let center_bin = order * fund_bin;

        if center_bin < magnitudes.len() {
            // Coherent sampling: the harmonic lands EXACTLY on center_bin
            let max_mag = magnitudes[center_bin];
            let ratio = max_mag / fundamental_mag;

            harmonics[order - 1] = ratio * 100.0;
            thd_sq_sum += ratio * ratio;
        }
    }

    let thd = crate::math::sqrt(thd_sq_sum) * 100.0;
    (harmonics, thd)
}

/// Computes spectral magnitudes from time-domain signal samples using 512-point Real FFT.
///
/// # Arguments
/// * `sync_buffer` - Mutable reference to the time-domain input sample buffer (FFT_RESOLUTION size).
/// * `magnitudes` - Output buffer to store scaled absolute real magnitudes.
///
/// # Returns
/// * `Option` containing the tuple of harmonic array percentages and THD percentage if fundamental is valid.
fn compute_magnitudes(
    sync_buffer: &mut [f32; FFT_RESOLUTION],
    magnitudes: &mut [f32; MAGNITUDES_LEN],
) -> Option<([f32; crate::types::NUMBER_HARMONICS], f32)> {
    // 1. Execute 512-point Real FFT (in-place execution via microfft)
    let spectrum = microfft::real::rfft_512(sync_buffer);

    let n = spectrum.len().min(MAGNITUDES_LEN);

    // Normalization factor for RFFT 512 points: 2.0 / N (where N = FFT_RESOLUTION)
    let scale_factor = 2.0 / (FFT_RESOLUTION as f32);

    // 2. Compute true physical RMS/Peak absolute magnitudes from complex bins
    // Bin 0 (DC component) does not carry the single-sided 2.0 multiplier factor
    magnitudes[0] =
        crate::math::sqrt(spectrum[0].re * spectrum[0].re + spectrum[0].im * spectrum[0].im)
            / (FFT_RESOLUTION as f32);

    // Bins 1..N (AC components and higher harmonics)
    for i in 1..n {
        let mag_raw =
            crate::math::sqrt(spectrum[i].re * spectrum[i].re + spectrum[i].im * spectrum[i].im);
        magnitudes[i] = mag_raw * scale_factor;
    }

    // 3. Search for the fundamental frequency bin.
    // Now that the buffer has already been resynchronized to exactly
    // CYCLES_PER_WINDOW cycles, fund_bin should always land on the
    // theoretical bin (CYCLES_PER_WINDOW). We still search a small window
    // around it as a safety net, instead of blindly scanning "the first 3
    // bins from bin 1" like before.
    let expected_bin = CYCLES_PER_WINDOW;
    let search_start = expected_bin.saturating_sub(1).max(1);
    let search_end = (expected_bin + 1).min(magnitudes.len() - 1);

    let (fund_bin, &fundamental_mag) = magnitudes
        .iter()
        .enumerate()
        .skip(search_start)
        .take(search_end - search_start + 1)
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(core::cmp::Ordering::Equal))?;

    // Threshold check to avoid division by zero or processing noise floor
    if fundamental_mag < FFT_MIN_FUNDAMENTAL_MAG {
        return None;
    }

    // 4. Compute relative spectrum percentages and THD
    Some(calculate_harmonics_and_thd(
        magnitudes,
        fund_bin,
        fundamental_mag,
    ))
}

/// Caches raw and resampled buffers used for FFT-based harmonic computation.
pub struct FftCache {
    /// Raw circular buffer, sampled at nominal FS (e.g. 8000 Hz), NOT synchronized.
    raw_buffer: Box<[f32; RAW_BUFFER_LEN]>,
    raw_write_pos: usize,
    raw_filled: bool,

    /// Already-resampled buffer: exactly CYCLES_PER_WINDOW cycles, ready for FFT.
    pub sync_buffer: Box<[f32; FFT_RESOLUTION]>,
    magnitudes: Box<[f32; MAGNITUDES_LEN]>,
    // NOTE: no windowing function here on purpose. Once the buffer is
    // coherently resampled to an exact integer number of cycles
    // (CYCLES_PER_WINDOW), the fundamental lands cleanly on a single bin
    // with no leakage. Applying a window (Hann, etc.) at that point is
    // counterproductive: windows exist specifically to reduce leakage when
    // you CANNOT guarantee coherent sampling, at the cost of spreading each
    // bin's energy into its neighbors. With CYCLES_PER_WINDOW = 1 the
    // fundamental sits at bin 1, right next to the H2/H3 bins — a Hann
    // window would leak fundamental energy straight into them, inflating
    // THD artificially (this was measured: removing the window dropped
    // THD-V from ~51% to the expected ~2% range).
}

impl FftCache {
    /// Creates a new `FftCache` with zeroed raw, sync and magnitude buffers.
    ///
    /// # Arguments
    ///
    /// * `_fft_len` - Unused; kept for API compatibility. The FFT size is fixed by `FFT_RESOLUTION`.
    ///
    /// # Returns
    ///
    /// A new `FftCache` instance.
    pub fn new(_fft_len: usize) -> Self {
        Self {
            raw_buffer: Box::new([0.0; RAW_BUFFER_LEN]),
            raw_write_pos: 0,
            raw_filled: false,
            sync_buffer: Box::new([0.0; FFT_RESOLUTION]),
            magnitudes: Box::new([0.0; MAGNITUDES_LEN]),
        }
    }

    /// Computes harmonics and THD directly on the `sync_buffer`
    /// previously resampled by the PLL/Resampler.
    pub fn compute_from_sync_buffer(
        &mut self,
    ) -> Option<([f32; crate::types::NUMBER_HARMONICS], f32)> {
        // 1. Remove the DC component from the resampled period
        remove_mean(self.sync_buffer.as_mut());

        // 2. Run the 512-point RFFT
        compute_magnitudes(self.sync_buffer.as_mut(), self.magnitudes.as_mut())
    }

    /// Feeds the raw circular buffer with a new sample at nominal FS.
    /// Call this once per ADC sample.
    pub fn push_raw_sample(&mut self, sample: f32) {
        self.raw_buffer[self.raw_write_pos] = sample;
        self.raw_write_pos = (self.raw_write_pos + 1) % RAW_BUFFER_LEN;
        if self.raw_write_pos == 0 {
            self.raw_filled = true;
        }
    }

    /// Extracts the last `n` samples from the circular buffer, in chronological order.
    fn extract_last_n(&self, n: usize) -> alloc::vec::Vec<f32> {
        let mut out = alloc::vec::Vec::with_capacity(n);
        let start = (self.raw_write_pos + RAW_BUFFER_LEN - n) % RAW_BUFFER_LEN;
        for k in 0..n {
            out.push(self.raw_buffer[(start + k) % RAW_BUFFER_LEN]);
        }
        out
    }

    /// Computes harmonics and THD, first resynchronizing the buffer to an
    /// integer number of cycles of the real measured frequency (`freq`),
    /// sampled at `fs`.
    ///
    /// This is what was missing before: freq/fs used to be received but
    /// never actually used (they were `_freq`, `_fs`).
    pub fn compute_harmonics_and_thd(
        &mut self,
        freq: f32,
        fs: f32,
    ) -> Option<([f32; crate::types::NUMBER_HARMONICS], f32)> {
        if freq <= 0.0 || fs <= 0.0 {
            return None;
        }

        // Number of raw samples that cover exactly CYCLES_PER_WINDOW cycles
        // of the real measured frequency.
        let raw_span = crate::math::round((CYCLES_PER_WINDOW as f32) * fs / freq) as usize;
        let raw_span = raw_span.clamp(2, RAW_BUFFER_LEN);

        // The circular buffer must already contain at least raw_span samples.
        if !self.raw_filled && self.raw_write_pos < raw_span {
            return None; // not enough samples accumulated yet
        }

        let raw_window = self.extract_last_n(raw_span);

        // Resample from raw_span samples (which cover exactly
        // CYCLES_PER_WINDOW real cycles) to FFT_RESOLUTION points.
        // After this, fund_bin = CYCLES_PER_WINDOW lands exactly, without leakage.
        let resampled = resample_signal(&raw_window, FFT_RESOLUTION);
        self.sync_buffer.copy_from_slice(&resampled);

        // Only remove DC offset. No windowing: the buffer is already
        // coherently synchronized to CYCLES_PER_WINDOW exact cycles, so a
        // rectangular window (i.e. no window at all) is the correct choice.
        remove_mean(self.sync_buffer.as_mut());

        compute_magnitudes(self.sync_buffer.as_mut(), self.magnitudes.as_mut())
    }
}

/// Incremental Goertzel-based accumulator for 49 interharmonic subgroup magnitudes.
///
/// Per IEC 61000-4-30 §5.9 (Class S: method left to manufacturer's discretion).
/// Each interharmonic subgroup i (0..49) covers the band between
/// harmonic (i+1) and (i+2), centered at (i+1.5) × fnominal.
///
/// ## Memory note
/// Instead of storing all 5120 samples (20 KB), this maintains only the Goertzel
/// filter state (49 × 2 f32 = 392 B) per accumulator. The computation runs
/// sample-by-sample as cycles are pushed.
///
/// ## CPU note
/// Each `push_cycle` processes 512 samples × 49 filters = 25 k iterations.
/// For 3 phases at 50 Hz ≈ 3.75 M ops/s ≈ 1.5 % of an ESP32-S3 at 240 MHz.
#[derive(Debug, Clone)]
pub struct InterharmonicAccumulator {
    /// Total samples accumulated so far (0..FFT_RESOLUTION × CYCLES_FOR_INTERHARMONIC)
    count: usize,
    /// Precomputed Goertzel coefficients: 2·cos(2π·fᵢ/fₛ)
    coeffs: [f32; INTERHARMONIC_GROUPS],
    /// Goertzel filter state q1[n-1] for each of 49 frequencies
    q1: [f32; INTERHARMONIC_GROUPS],
    /// Goertzel filter state q2[n-2] for each of 49 frequencies
    q2: [f32; INTERHARMONIC_GROUPS],
}

impl InterharmonicAccumulator {
    /// Creates a new accumulator, precomputing the Goertzel coefficients for the
    /// 49 interharmonic subgroup center frequencies at the given sync sample rate.
    ///
    /// # Arguments
    ///
    /// * `fs_sync` - Sample rate (Hz) of the sync-resampled data.
    ///
    /// # Returns
    ///
    /// A new `InterharmonicAccumulator` with zeroed filter state.
    pub fn new(fs_sync: f32) -> Self {
        let mut coeffs = [0.0; INTERHARMONIC_GROUPS];
        for (i, c) in coeffs.iter_mut().enumerate() {
            let f_center = (i as f32 + 1.5) * crate::FREQ_NOMINAL_50;
            *c = 2.0 * libm::cosf(core::f32::consts::TAU * f_center / fs_sync);
        }
        Self {
            count: 0,
            coeffs,
            q1: [0.0; INTERHARMONIC_GROUPS],
            q2: [0.0; INTERHARMONIC_GROUPS],
        }
    }

    /// Push one cycle of sync-resampled data (exactly `FFT_RESOLUTION` samples).
    /// Runs all 49 Goertzel filters incrementally on each sample.
    /// Call once per cycle for each phase voltage.
    pub fn push_cycle(&mut self, sync_data: &[f32; FFT_RESOLUTION]) {
        let remaining = (FFT_RESOLUTION * CYCLES_FOR_INTERHARMONIC).saturating_sub(self.count);
        let n = remaining.min(FFT_RESOLUTION);
        for &sample in sync_data[..n].iter() {
            for i in 0..INTERHARMONIC_GROUPS {
                let q0 = sample + self.coeffs[i] * self.q1[i] - self.q2[i];
                self.q2[i] = self.q1[i];
                self.q1[i] = q0;
            }
        }
        self.count += n;
    }

    /// Returns whether enough samples have been accumulated for a valid computation.
    ///
    /// # Returns
    ///
    /// `true` once `FFT_RESOLUTION × CYCLES_FOR_INTERHARMONIC` samples have been pushed.
    pub fn is_ready(&self) -> bool {
        self.count >= FFT_RESOLUTION * CYCLES_FOR_INTERHARMONIC
    }

    /// Resets the accumulator count and Goertzel filter state.
    pub fn reset(&mut self) {
        self.count = 0;
        self.q1 = [0.0; INTERHARMONIC_GROUPS];
        self.q2 = [0.0; INTERHARMONIC_GROUPS];
    }

    /// Compute 49 interharmonic subgroup magnitudes as percentage of fundamental
    /// from the current Goertzel filter state. Resets the accumulator.
    /// Returns None if not enough samples accumulated.
    pub fn compute(&mut self, fundamental_mag: f32) -> Option<[f32; INTERHARMONIC_GROUPS]> {
        if !self.is_ready() {
            return None;
        }
        let n = self.count as f32;
        let mut result = [0.0; INTERHARMONIC_GROUPS];

        for (i, res) in result.iter_mut().enumerate() {
            let power = self.q1[i] * self.q1[i] + self.q2[i] * self.q2[i]
                - self.coeffs[i] * self.q1[i] * self.q2[i];
            let mag = crate::math::sqrt(power / (n * n)) * 2.0;
            *res = if fundamental_mag > FFT_MIN_FUNDAMENTAL_MAG {
                (mag / fundamental_mag) * 100.0
            } else {
                0.0
            };
        }

        self.count = 0;
        self.q1 = [0.0; INTERHARMONIC_GROUPS];
        self.q2 = [0.0; INTERHARMONIC_GROUPS];
        Some(result)
    }
}

/// Removes the DC component by subtracting the mean value from every sample.
///
/// # Arguments
///
/// * `signal` - Samples to subtract the mean from, modified in place.
fn remove_mean(signal: &mut [f32]) {
    let mean = signal.iter().sum::<f32>() / signal.len() as f32;
    for sample in signal.iter_mut() {
        *sample -= mean;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generates `n` samples of a sine wave of the given frequency, sample rate and amplitude.
    ///
    /// # Arguments
    ///
    /// * `freq` - Sine wave frequency in Hz.
    /// * `fs` - Sample rate in Hz.
    /// * `n` - Number of samples to generate.
    /// * `amp` - Peak amplitude of the sine wave.
    ///
    /// # Returns
    ///
    /// A vector of `n` sine wave samples.
    fn generate_sine(freq: f32, fs: f32, n: usize, amp: f32) -> alloc::vec::Vec<f32> {
        (0..n)
            .map(|i| {
                let t = i as f32 / fs;
                crate::math::sin(core::f32::consts::TAU * freq * t) * amp
            })
            .collect()
    }

    /// Verifies that a clean 50 Hz sine produces near-zero interharmonic magnitudes in all bands.
    #[test]
    fn test_interharmonic_accumulator_clean_sine() {
        let fs_sync = 25600.0;
        let mut acc = InterharmonicAccumulator::new(fs_sync);
        assert!(!acc.is_ready());

        let freq = 50.0;
        let amp = 230.0;
        let n = FFT_RESOLUTION;

        for _ in 0..CYCLES_FOR_INTERHARMONIC {
            let samples = generate_sine(freq, fs_sync, n, amp);
            let mut buf = [0.0; FFT_RESOLUTION];
            buf.copy_from_slice(&samples);
            acc.push_cycle(&buf);
        }

        assert!(acc.is_ready());
        let result = acc.compute(amp).unwrap();
        // Clean 50 Hz sine → all interharmonic bands should be near zero
        for (i, &val) in result.iter().enumerate() {
            assert!(val < 0.1, "Interharmonic group {} too high: {}%", i, val);
        }
    }

    /// Verifies that a 75 Hz interharmonic component is detected in its expected band (~1%).
    #[test]
    fn test_interharmonic_accumulator_with_75hz() {
        let fs_sync = 25600.0;
        let mut acc = InterharmonicAccumulator::new(fs_sync);

        let f_fund = 50.0;
        let f_inter = 75.0;
        let amp = 230.0;
        let inter_amp = 2.3; // 1% interharmonic

        // Generate one continuous 5120-sample buffer across all cycles
        let total_n = FFT_RESOLUTION * CYCLES_FOR_INTERHARMONIC;
        let full_signal: alloc::vec::Vec<f32> = (0..total_n)
            .map(|i| {
                let t = i as f32 / fs_sync;
                crate::math::sin(core::f32::consts::TAU * f_fund * t) * amp
                    + crate::math::sin(core::f32::consts::TAU * f_inter * t) * inter_amp
            })
            .collect();

        for c in 0..CYCLES_FOR_INTERHARMONIC {
            let mut buf = [0.0; FFT_RESOLUTION];
            let offset = c * FFT_RESOLUTION;
            buf.copy_from_slice(&full_signal[offset..offset + FFT_RESOLUTION]);
            acc.push_cycle(&buf);
        }

        let result = acc.compute(amp).unwrap();
        // Group 0 (between 50-100 Hz, center 75 Hz) should be ~1%
        assert!(
            (result[0] - 1.0).abs() < 0.3,
            "Group 0 (75 Hz) expected ~1%, got {}%",
            result[0]
        );
        // All other groups should be small
        for (i, &val) in result.iter().enumerate().skip(1) {
            assert!(val < 0.5, "Interharmonic group {} too high: {}%", i, val);
        }
    }
}
