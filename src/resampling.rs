//! Signal resampling utilities (linear and synchronous).
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

/// Resample a signal into a fixed number of points using linear interpolation.
///
/// Writes up to `target_points` interpolated samples into `output`, applying an
/// optional phase delay.
///
/// The number of points written to `output`.
pub fn resample_synchronous_into(
    input: &[f32],
    _fs: f32,
    _freq_est: f32,
    _num_cycles: usize,
    target_points: usize,
    phase_delay_us: f32,
    output: &mut [f32],
) -> usize {
    let n = target_points.min(output.len());
    if input.is_empty() {
        for val in output[..n].iter_mut() {
            *val = 0.0;
        }
        return n;
    }

    let step = input.len() as f32 / target_points as f32;
    let phase_offset = phase_delay_us * 1e-6 * input.len() as f32 / (target_points as f32);

    for (i, out) in output[..n].iter_mut().enumerate() {
        let pos = (i as f32 * step) + phase_offset;
        let idx0 = crate::math::floor(pos) as usize;
        let idx1 = (idx0 + 1).min(input.len() - 1);
        let frac = pos - idx0 as f32;
        let y0 = input[idx0];
        let y1 = input[idx1];
        *out = y0 + (y1 - y0) * frac;
    }
    n
}

/// Resample a signal into a new vector of `target_points` samples.
#[cfg(feature = "alloc")]
pub fn resample_synchronous(
    input: &[f32],
    fs: f32,
    freq_est: f32,
    num_cycles: usize,
    target_points: usize,
    phase_delay_us: f32,
) -> alloc::vec::Vec<f32> {
    let mut output = alloc::vec![0.0; target_points];
    let written = resample_synchronous_into(
        input,
        fs,
        freq_est,
        num_cycles,
        target_points,
        phase_delay_us,
        &mut output,
    );
    output.truncate(written);
    output
}
