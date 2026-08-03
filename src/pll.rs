//! High-inertia digital PLL for grid frequency tracking and synchronization.
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

use super::types::{PllConfig, PllState};
use core::f32::consts::PI;

pub const TWO_PI: f32 = 2.0 * PI;

// Minimum signal amplitude required for PLL phase-error normalization
pub const PLL_NORM_THRESHOLD: f32 = 0.001;
// Integrator anti-windup clamp (±rad/s correction limit)
pub const PLL_INTEGRATOR_CLAMP: f32 = 0.1;
// EMA alpha for lock-error accumulator (lower = slower, more stable lock detection)
pub const PLL_LOCK_EMA_ALPHA: f32 = 0.01;
// Complementary weight for lock EMA (1 - PLL_LOCK_EMA_ALPHA)
const PLL_LOCK_EMA_DECAY: f32 = 1.0 - PLL_LOCK_EMA_ALPHA;
// Minimum accumulated error below which PLL is considered locked
pub const PLL_ERROR_ACCUM_THRESHOLD: f32 = 0.1;

/// Update the PLL state using a batch of signal samples.
///
/// Adjusts the estimated frequency and phase per sample, then updates the lock
/// status and the 10-second frequency average.
///
/// # Arguments
///
/// * `state` — Mutable PLL state to update.
/// * `samples` — Signal samples to process.
/// * `fs` — Sampling rate in Hz.
/// * `nominal_freq` — Nominal system frequency in Hz.
/// * `cfg` — PLL configuration parameters.
pub fn update_pll(
    state: &mut PllState,
    samples: &[f32],
    fs: f32,
    nominal_freq: f32,
    cfg: &PllConfig,
) {
    let ts = 1.0 / fs;

    if state.freq_est == 0.0 {
        state.freq_est = nominal_freq;
    }

    for &sample in samples {
        let input_norm = if sample.abs() > cfg.norm_threshold {
            sample.signum()
        } else {
            0.0
        };

        let phase_error = -crate::math::sin(state.phase) * input_norm;

        state.integrator += cfg.ki * phase_error;
        state.integrator = state
            .integrator
            .clamp(-cfg.integrator_clamp, cfg.integrator_clamp);
        let freq_corr = cfg.kp * phase_error + state.integrator;

        state.freq_est = nominal_freq + freq_corr;

        state.freq_est = state.freq_est.clamp(cfg.freq_min, cfg.freq_max);

        state.phase += TWO_PI * state.freq_est * ts;

        if state.phase > TWO_PI {
            state.phase -= TWO_PI;
        }
    }

    state.error_accum = state.error_accum * PLL_LOCK_EMA_DECAY
        + (nominal_freq - state.freq_est).abs() * cfg.lock_ema_alpha;
    state.locked = state.error_accum < cfg.lock_threshold;

    // Update 10-second moving average ring buffer using 1-second bins (EN 61000-4-30 Sec 5.1 requirement)
    state.cycle_freq_sum += state.freq_est;
    state.cycle_freq_count += 1;

    let cycles_per_sec = (crate::math::round(nominal_freq) as usize).max(1);
    if state.cycle_freq_count >= cycles_per_sec {
        let avg_1s = state.cycle_freq_sum / (state.cycle_freq_count as f32);
        state.freq_buf[state.freq_buf_idx] = avg_1s;
        state.freq_buf_idx = (state.freq_buf_idx + 1) % 10;
        if state.freq_buf_count < 10 {
            state.freq_buf_count += 1;
        }
        state.cycle_freq_sum = 0.0;
        state.cycle_freq_count = 0;
    }

    if state.freq_buf_count > 0 {
        let sum: f32 = state.freq_buf[..state.freq_buf_count].iter().sum();
        state.freq_10s = sum / state.freq_buf_count as f32;
    } else {
        state.freq_10s = state.freq_est;
    }
}
