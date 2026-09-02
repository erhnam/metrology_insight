//! Power metrics computation (real, reactive, apparent and power factor).
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

use crate::{MetrologyInsightSignal, MetrologyInsightSocket, PowerMetrics};

#[allow(dead_code)]
/// Compute real power from RMS voltage, RMS current, and power factor.
///
/// # Arguments
///
/// * `voltage_rms` — RMS voltage in volts.
/// * `current_rms` — RMS current in amperes.
/// * `power_factor` — Power factor (cos φ).
///
/// # Returns
///
/// The product `voltage × current × power factor` in watts.
fn real_power_from_rms_and_power_factor(
    voltage_rms: f32,
    current_rms: f32,
    power_factor: f32,
) -> f32 {
    voltage_rms * current_rms * power_factor
}

/// Compute real power as the average of the instantaneous voltage–current product.
///
/// # Arguments
///
/// * `signal_v` — Voltage samples.
/// * `signal_i` — Current samples.
///
/// # Returns
///
/// The average instantaneous power, or 0.0 when the signals are empty or have
/// different lengths.
fn real_power_from_signals(signal_v: &[f32], signal_i: &[f32]) -> f32 {
    if signal_v.is_empty() || signal_v.len() != signal_i.len() {
        return 0.0;
    }
    signal_v
        .iter()
        .zip(signal_i.iter())
        .map(|(&v, &i)| v * i)
        .sum::<f32>()
        / signal_v.len() as f32
}

/// Compute fundamental reactive power from apparent power and voltage-to-current phase angle (IEEE 1459 / IEC 62053-23).
///
/// Q = S * sin(phi)
///
/// Robust across all four quadrants and immune to division-by-zero or collapse when real power P is 0.
fn reactive_power_from_angle(_real_power: f32, apparent_power: f32, c2v_angle_deg: f32) -> f32 {
    apparent_power * crate::math::sin(c2v_angle_deg.to_radians())
}

/// Compute apparent power as the product of RMS voltage and RMS current.
///
/// # Arguments
///
/// * `voltage_rms` — RMS voltage in volts.
/// * `current_rms` — RMS current in amperes.
///
/// # Returns
///
/// The apparent power in volt-amperes.
fn apparent_power_from_rms(voltage_rms: f32, current_rms: f32) -> f32 {
    voltage_rms * current_rms
}

/// Compute the power factor as the ratio of real to apparent power.
///
/// # Arguments
///
/// * `apparent_power` — Apparent power in volt-amperes.
/// * `real_power` — Real power in watts.
///
/// # Returns
///
/// The power factor clamped to the range [-1.0, 1.0], or 0.0 when the apparent
/// power is zero.
fn power_factor_from_apparent_and_real(apparent_power: f32, real_power: f32) -> f32 {
    if apparent_power.abs() > 0.0 {
        (real_power / apparent_power).clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

/// Calculate all power metrics (real, reactive, apparent, and power factor) for
/// a single phase.
///
/// # Arguments
///
/// * `voltage_signal` — Mutable voltage signal used to read the real-wave slice.
/// * `current_signal` — Mutable current signal used to read the real-wave slice.
/// * `c2v_angle` — Current-to-voltage phase angle in degrees.
///
/// # Returns
///
/// A [`PowerMetrics`] struct containing the computed power values.
fn calculate_all_power_metrics(
    voltage_signal: &mut MetrologyInsightSignal,
    current_signal: &mut MetrologyInsightSignal,
    c2v_angle: f32,
) -> PowerMetrics {
    let real_power = real_power_from_signals(
        voltage_signal.real_wave_slice(),
        current_signal.real_wave_slice(),
    );

    let apparent_power = apparent_power_from_rms(voltage_signal.rms, current_signal.rms);

    let reactive_power = reactive_power_from_angle(real_power, apparent_power, c2v_angle);

    let power_factor_calc = power_factor_from_apparent_and_real(apparent_power, real_power);
    let displacement_pf = crate::math::cos(c2v_angle.to_radians());

    PowerMetrics {
        real_power,
        reactive_power,
        apparent_power,
        power_factor: power_factor_calc,
        displacement_pf,
    }
}

/// Update the per-phase and total power metrics across all active phases.
///
/// # Arguments
///
/// * `socket` — Mutable metrology socket whose phase and total power metrics are updated.
/// * `active_phases` — Number of active phases to process.
pub fn update_power_metrics(socket: &mut MetrologyInsightSocket, active_phases: usize) {
    for i in 0..active_phases {
        let c2v_angle = socket.phases[i].phase_angles.c2v_angle;
        socket.phases[i].power_metrics = calculate_all_power_metrics(
            &mut socket.phases[i].voltage,
            &mut socket.phases[i].current,
            c2v_angle,
        );
    }

    let mut total_real: f32 = 0.0;
    let mut total_react: f32 = 0.0;
    for i in 0..active_phases {
        total_real += socket.phases[i].power_metrics.real_power;
        total_react += socket.phases[i].power_metrics.reactive_power;
    }
    let total_apparent = libm::sqrtf(total_real * total_real + total_react * total_react);
    let total_pf = if total_apparent > 0.0 {
        total_real / total_apparent
    } else {
        0.0
    };

    socket.power_metrics_total = PowerMetrics {
        real_power: total_real,
        reactive_power: total_react,
        apparent_power: total_apparent,
        power_factor: total_pf.clamp(-1.0, 1.0),
        displacement_pf: 0.0, // Not typically aggregated simply for total, or can be cos(atan(Q/P))
    };
    if socket.power_metrics_total.real_power.abs() > 0.0 {
        socket.power_metrics_total.displacement_pf = crate::math::cos(crate::math::atan2(
            socket.power_metrics_total.reactive_power,
            socket.power_metrics_total.real_power,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pure_reactive_power() {
        let apparent = 1000.0;
        // Pure inductive (+90 deg)
        let q_ind = reactive_power_from_angle(0.0, apparent, 90.0);
        assert!(
            (q_ind - 1000.0).abs() < 1e-3,
            "Expected 1000 VAR, got {}",
            q_ind
        );

        // Pure capacitive (-90 deg)
        let q_cap = reactive_power_from_angle(0.0, apparent, -90.0);
        assert!(
            (q_cap - -1000.0).abs() < 1e-3,
            "Expected -1000 VAR, got {}",
            q_cap
        );

        // In-phase (0 deg)
        let q_zero = reactive_power_from_angle(1000.0, apparent, 0.0);
        assert!(q_zero.abs() < 1e-3, "Expected 0 VAR, got {}", q_zero);
    }
}
