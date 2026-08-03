//! Phase angle measurement and direction classification (inductive/capacitive/in-phase).
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

use crate::{MetrologyInsightSocket, PhaseAngleMetrics, PhaseDirection};

// Dead-band for phase direction classification (degrees).
// Angles within ±PHASE_DIRECTION_DEADBAND_DEG are classified as InPhase.
pub const PHASE_DIRECTION_DEADBAND_DEG: f32 = 0.5;

// Unused helper: Alternative power-factor based phase angle calculator. Retained for API completeness.
/// Computes the phase angle (degrees) from power factor and reactive power sign.
///
/// # Arguments
///
/// * `power_factor` - Power factor in the range [-1, 1].
/// * `react_power` - Reactive power; its sign determines the angle polarity.
///
/// # Returns
///
/// The phase angle in degrees, or 0.0 if `power_factor` is out of range.
#[allow(dead_code)]
fn phase_angle_from_pf_and_react_power(power_factor: f32, react_power: f32) -> f32 {
    if !(-1.0..=1.0).contains(&power_factor) {
        return 0.0;
    }

    let clamped_pf = power_factor.clamp(-1.0, 1.0);
    let mut phase_rad = crate::math::acos(clamped_pf);

    if react_power < 0.0 {
        phase_rad = -phase_rad;
    }

    phase_rad.to_degrees()
}

// Unused helper: Calculates unsigned phase angle via vector dot product (acos).
// Retained for reference, but unused in telemetry pipeline because acos() always returns
// non-negative values [0, 180] deg, preventing capacitive (negative phase angle) detection.
/// Computes the unsigned phase angle (degrees) via the vector dot product (acos).
///
/// # Arguments
///
/// * `voltage` - Voltage samples.
/// * `current` - Current samples.
///
/// # Returns
///
/// The unsigned phase angle in degrees, always within [0, 180].
#[allow(dead_code)]
fn phase_angle_from_signals(voltage: &[f32], current: &[f32]) -> f32 {
    let dot: f32 = voltage.iter().zip(current.iter()).map(|(v, i)| v * i).sum();
    let v_mag: f32 = crate::math::sqrt(voltage.iter().map(|v| v * v).sum::<f32>());
    let i_mag: f32 = crate::math::sqrt(current.iter().map(|i| i * i).sum::<f32>());

    if v_mag == 0.0 || i_mag == 0.0 {
        return 0.0;
    }

    let cos_phi = (dot / (v_mag * i_mag)).clamp(-1.0, 1.0);
    crate::math::acos(cos_phi).to_degrees()
}

/// Finds the first rising zero crossing, linearly interpolated for sub-sample accuracy.
///
/// # Arguments
///
/// * `signal` - Signal samples.
///
/// # Returns
///
/// The interpolated sample index of the first rising zero crossing, or `None` if none exists.
fn find_first_rising_zero_crossing(signal: &[f32]) -> Option<f32> {
    for i in 1..signal.len() {
        if signal[i - 1] < 0.0 && signal[i] >= 0.0 {
            let y1 = signal[i - 1];
            let y2 = signal[i];
            if (y2 - y1).abs() > f32::EPSILON {
                return Some((i - 1) as f32 + (0.0 - y1) / (y2 - y1));
            }
            return Some(i as f32);
        }
    }
    None
}

/// Converts a sample index to a phase angle in degrees within one cycle.
///
/// # Arguments
///
/// * `sample_index` - Sample index within the cycle.
/// * `samples_per_cycle` - Number of samples per cycle.
///
/// # Returns
///
/// The equivalent angle in degrees in the range [0, 360).
fn sample_index_to_angle(sample_index: f32, samples_per_cycle: f32) -> f32 {
    let angle_deg = (sample_index / samples_per_cycle) * 360.0;
    angle_deg % 360.0
}

/// Computes the absolute zero-crossing phase angles of the voltage and current signals.
///
/// # Arguments
///
/// * `voltage_signal` - Voltage samples.
/// * `current_signal` - Current samples.
/// * `adc_samples_second` - ADC sampling rate in samples per second.
/// * `freq_est` - Estimated signal frequency in Hz.
///
/// # Returns
///
/// A tuple `(v_angle, c_angle)` with the voltage and current phase angles in degrees.
fn absolute_phase_angles_from_signals(
    voltage_signal: &[f32],
    current_signal: &[f32],
    adc_samples_second: f32,
    freq_est: f32,
) -> (f32, f32) {
    let v_index = find_first_rising_zero_crossing(voltage_signal).unwrap_or(0.0);
    let c_index = find_first_rising_zero_crossing(current_signal).unwrap_or(0.0);
    let samples_per_cycle = adc_samples_second / freq_est;

    let v_angle = sample_index_to_angle(v_index, samples_per_cycle);
    let c_angle = sample_index_to_angle(c_index, samples_per_cycle);

    (v_angle, c_angle)
}

/// Computes the full phase-angle metrics, including the signed current-to-voltage angle and direction.
///
/// # Arguments
///
/// * `voltage_signal` - Voltage samples.
/// * `current_signal` - Current samples.
/// * `adc_samples_second` - ADC sampling rate in samples per second.
/// * `freq_est` - Estimated signal frequency in Hz.
///
/// # Returns
///
/// A `PhaseAngleMetrics` struct with the angles and classified direction.
fn all_phase_angles_from_signals(
    voltage_signal: &[f32],
    current_signal: &[f32],
    adc_samples_second: f32,
    freq_est: f32,
) -> PhaseAngleMetrics {
    let (v_angle, c_angle) = absolute_phase_angles_from_signals(
        voltage_signal,
        current_signal,
        adc_samples_second,
        freq_est,
    );

    // Signed angle difference: c_angle - v_angle normalized to (-180, +180]
    let mut c2v_angle = c_angle - v_angle;
    if c2v_angle > 180.0 {
        c2v_angle -= 360.0;
    } else if c2v_angle <= -180.0 {
        c2v_angle += 360.0;
    }

    let direction = if c2v_angle > PHASE_DIRECTION_DEADBAND_DEG {
        PhaseDirection::Inductive
    } else if c2v_angle < -PHASE_DIRECTION_DEADBAND_DEG {
        PhaseDirection::Capacitive
    } else {
        PhaseDirection::InPhase
    };

    PhaseAngleMetrics {
        c2v_angle,
        v_angle,
        c_angle,
        direction,
    }
}

/// Updates the phase-angle metrics for every active phase of the socket.
///
/// # Arguments
///
/// * `socket` - Metrology socket whose per-phase metrics are updated in place.
/// * `adc_samples_second` - ADC sampling rate in samples per second.
/// * `active_phases` - Number of phases to process.
pub fn update_phase_angles(
    socket: &mut MetrologyInsightSocket,
    adc_samples_second: f32,
    active_phases: usize,
) {
    for i in 0..active_phases {
        let freq_est = socket.phases[i].voltage.pll_state.freq_est;
        if freq_est <= 0.0 {
            continue;
        }
        socket.phases[i].phase_angles = all_phase_angles_from_signals(
            socket.phases[i].voltage.real_wave_slice(),
            socket.phases[i].current.real_wave_slice(),
            adc_samples_second,
            freq_est,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::f32::consts::PI;

    /// Verifies phase direction classification for in-phase, inductive and capacitive signals.
    #[test]
    fn test_phase_direction_inductive_capacitive_inphase() {
        let fs = 8000.0;
        let freq = 50.0;
        let n_samples = 160;

        // In-phase signals
        let v_in: Vec<f32> = (0..n_samples)
            .map(|i| crate::math::sin(2.0 * PI * freq * (i as f32 / fs)))
            .collect();
        let i_in: Vec<f32> = (0..n_samples)
            .map(|i| crate::math::sin(2.0 * PI * freq * (i as f32 / fs)))
            .collect();
        let res_in = all_phase_angles_from_signals(&v_in, &i_in, fs, freq);
        assert!(matches!(res_in.direction, PhaseDirection::InPhase));

        // Inductive signal: Current lags Voltage by 30 deg (shift current by -30 deg)
        let i_ind: Vec<f32> = (0..n_samples)
            .map(|i| crate::math::sin(2.0 * PI * freq * (i as f32 / fs) - 30.0 * PI / 180.0))
            .collect();
        let res_ind = all_phase_angles_from_signals(&v_in, &i_ind, fs, freq);
        assert!(matches!(res_ind.direction, PhaseDirection::Inductive));

        // Capacitive signal: Current leads Voltage by 30 deg (shift current by +30 deg)
        let i_cap: Vec<f32> = (0..n_samples)
            .map(|i| crate::math::sin(2.0 * PI * freq * (i as f32 / fs) + 30.0 * PI / 180.0))
            .collect();
        let res_cap = all_phase_angles_from_signals(&v_in, &i_cap, fs, freq);
        assert!(matches!(res_cap.direction, PhaseDirection::Capacitive));
    }
}
