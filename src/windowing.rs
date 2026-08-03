//! Window function generation (Hann) for spectral analysis.
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

/// Fill `window` with a Hann window function in-place.
///
/// Coefficients: w[i] = 0.5 × (1 − cos(2π × i / (N−1))).
///
/// # Arguments
///
/// * `window` — Mutable slice to be filled with window coefficients.
pub fn hann(window: &mut [f32]) {
    let n = window.len();
    for (i, w) in window.iter_mut().enumerate() {
        let x = core::f32::consts::TAU * i as f32 / (n - 1) as f32;
        *w = 0.5 * (1.0 - libm::cosf(x));
    }
}

/// Fill `window` with a 4-term Blackman-Harris window function in-place.
///
/// Coefficients (a0=0.35875, a1=0.48829, a2=0.14128, a3=0.01168):
/// w[i] = a0 − a1·cos(2π·i/N) + a2·cos(4π·i/N) − a3·cos(6π·i/N)
///
/// This window provides good sidelobe suppression (−92 dB) at the
/// cost of a wider main lobe. Recommended for harmonic analysis.
///
/// # Arguments
///
/// * `window` — Mutable slice to be filled with window coefficients.
pub fn blackman_harris(window: &mut [f32]) {
    let n = window.len();
    let a0 = 0.35875;
    let a1 = 0.48829;
    let a2 = 0.14128;
    let a3 = 0.01168;

    for (i, w) in window.iter_mut().enumerate() {
        let x = core::f32::consts::TAU * i as f32 / (n - 1) as f32;
        *w = a0 - a1 * libm::cosf(x) + a2 * libm::cosf(2.0 * x) - a3 * libm::cosf(3.0 * x);
    }
}
