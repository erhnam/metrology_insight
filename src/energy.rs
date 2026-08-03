//! Active and reactive energy computation with 4-quadrant micro-joule accumulation.
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

use crate::{ActiveEnergyMetrics, EnergyMetrics, MetrologyInsightSocket, ReactiveEnergyMetrics};

const JOULES_TO_KWH: f64 = 1.0 / (3600.0 * 1000.0);
const W_SEC_TO_UJ: f64 = 1_000_000.0;

/// Compute the elapsed time covered by the buffered voltage samples.
///
/// # Arguments
///
/// * `socket` — Metrology socket whose first phase's buffered sample count is used.
/// * `adc_samples_second` — ADC sampling rate in samples per second.
///
/// # Returns
///
/// The elapsed time in seconds, or [`None`] when no samples are buffered or the
/// sampling rate is zero.
fn elapsed_time_seconds(socket: &MetrologyInsightSocket, adc_samples_second: f64) -> Option<f64> {
    let samples_count = socket.phases[0].voltage.real_wave_len as f64;
    if samples_count == 0.0 || adc_samples_second == 0.0 {
        None
    } else {
        let sample_duration = 1.0 / adc_samples_second;
        Some(samples_count * sample_duration)
    }
}

/// Accumulate active energy into the quadrant determined by the signs of real
/// and reactive power, then refresh the corresponding kWh values.
///
/// # Arguments
///
/// * `socket` — Mutable metrology socket whose quadrant energy counters are updated.
/// * `adc_samples_second` — ADC sampling rate in samples per second.
fn active_energy_by_quadrant(socket: &mut MetrologyInsightSocket, adc_samples_second: f64) {
    if let Some(elapsed_time) = elapsed_time_seconds(socket, adc_samples_second) {
        let p_real = socket.power_metrics_total.real_power as f64;
        let p_react = socket.power_metrics_total.reactive_power as f64;
        let delta_uj = (p_real.abs() * elapsed_time * W_SEC_TO_UJ) as i128;

        if p_real > 0.0 {
            if p_react >= 0.0 { socket.energy_metrics.active.q1_uj += delta_uj; }
            else { socket.energy_metrics.active.q4_uj += delta_uj; }
        } else if p_real < 0.0 {
            if p_react >= 0.0 { socket.energy_metrics.active.q2_uj += delta_uj; }
            else { socket.energy_metrics.active.q3_uj += delta_uj; }
        }

        let factor = JOULES_TO_KWH / W_SEC_TO_UJ;
        socket.energy_metrics.active.q1 = socket.energy_metrics.active.q1_uj as f64 * factor;
        socket.energy_metrics.active.q2 = socket.energy_metrics.active.q2_uj as f64 * factor;
        socket.energy_metrics.active.q3 = socket.energy_metrics.active.q3_uj as f64 * factor;
        socket.energy_metrics.active.q4 = socket.energy_metrics.active.q4_uj as f64 * factor;
    }
}

/// Accumulate reactive energy into the quadrant determined by the signs of real
/// and reactive power, then refresh the corresponding kWh values.
///
/// # Arguments
///
/// * `socket` — Mutable metrology socket whose quadrant energy counters are updated.
/// * `adc_samples_second` — ADC sampling rate in samples per second.
fn reactive_energy_by_quadrant(socket: &mut MetrologyInsightSocket, adc_samples_second: f64) {
    if let Some(elapsed_time) = elapsed_time_seconds(socket, adc_samples_second) {
        let p_real = socket.power_metrics_total.real_power as f64;
        let p_react = socket.power_metrics_total.reactive_power as f64;
        let delta_uj = (p_react.abs() * elapsed_time * W_SEC_TO_UJ) as i128;

        if p_real >= 0.0 {
            if p_react > 0.0 { socket.energy_metrics.reactive.q1_uj += delta_uj; }
            else if p_react < 0.0 { socket.energy_metrics.reactive.q4_uj += delta_uj; }
        } else {
            if p_react > 0.0 { socket.energy_metrics.reactive.q2_uj += delta_uj; }
            else if p_react < 0.0 { socket.energy_metrics.reactive.q3_uj += delta_uj; }
        }

        let factor = JOULES_TO_KWH / W_SEC_TO_UJ;
        socket.energy_metrics.reactive.q1 = socket.energy_metrics.reactive.q1_uj as f64 * factor;
        socket.energy_metrics.reactive.q2 = socket.energy_metrics.reactive.q2_uj as f64 * factor;
        socket.energy_metrics.reactive.q3 = socket.energy_metrics.reactive.q3_uj as f64 * factor;
        socket.energy_metrics.reactive.q4 = socket.energy_metrics.reactive.q4_uj as f64 * factor;
    }
}

/// Update both the active and reactive quadrant energy counters.
///
/// # Arguments
///
/// * `socket` — Mutable metrology socket whose quadrant energy metrics are updated.
/// * `adc_samples_second` — ADC sampling rate in samples per second.
pub fn update_energy_by_quadrant(socket: &mut MetrologyInsightSocket, adc_samples_second: f64) {
    active_energy_by_quadrant(socket, adc_samples_second);
    reactive_energy_by_quadrant(socket, adc_samples_second);
}

/// Update quadrant energy and recompute the total imported, exported, and
/// balance energy values for both active and reactive energy.
///
/// # Arguments
///
/// * `socket` — Mutable metrology socket whose total energy metrics are updated.
/// * `adc_samples_second` — ADC sampling rate in samples per second.
/// * `_active_phases` — Number of active phases (currently unused).
pub fn update_total_energy(socket: &mut MetrologyInsightSocket, adc_samples_second: f64, _active_phases: usize) {
    update_energy_by_quadrant(socket, adc_samples_second);

    let active = &mut socket.energy_metrics.active;
    let reactive = &mut socket.energy_metrics.reactive;

    socket.energy_metrics = EnergyMetrics {
        active: ActiveEnergyMetrics {
            imported: active.imported(),
            exported: active.exported(),
            balance: active.balance(),
            ..active.clone()
        },
        reactive: ReactiveEnergyMetrics {
            inductive: reactive.inductive(),
            capacitive: reactive.capacitive(),
            balance: reactive.balance(),
            ..reactive.clone()
        },
    }
}
