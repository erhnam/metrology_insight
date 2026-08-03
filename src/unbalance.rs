//! Voltage and current unbalance via Fortescue symmetrical components.
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

use num_complex::Complex;
use serde::{Deserialize, Serialize};

/// Voltage and current unbalance metrics computed via the Fortescue method.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct UnbalanceMetrics {
    // Voltage symmetrical components
    pub v0_zero_seq: f32,       // Zero sequence voltage magnitude (V)
    pub v1_pos_seq: f32,        // Positive sequence voltage magnitude (V)
    pub v2_neg_seq: f32,        // Negative sequence voltage magnitude (V)
    pub u2_neg_ratio_pct: f32,  // Negative sequence voltage unbalance u2 (%)
    pub u0_zero_ratio_pct: f32, // Zero sequence voltage unbalance u0 (%)
    // Current symmetrical components (§5.13.6)
    pub i0_zero_seq: f32,    // Zero sequence current magnitude (A)
    pub i1_pos_seq: f32,     // Positive sequence current magnitude (A)
    pub i2_neg_seq: f32,     // Negative sequence current magnitude (A)
    pub u2_i_ratio_pct: f32, // Negative sequence current unbalance (%)
    pub u0_i_ratio_pct: f32, // Zero sequence current unbalance (%)
}

/// Fortescue operator a = e^(j·120°)
fn fortescue_ops() -> (Complex<f32>, Complex<f32>) {
    let a = Complex::from_polar(1.0, (120.0_f32).to_radians());
    (a, a * a) // a, a²
}

/// Compute symmetrical components (zero, positive, negative) from three phasors.
fn symmetrical_components(
    pa: Complex<f32>,
    pb: Complex<f32>,
    pc: Complex<f32>,
) -> (Complex<f32>, Complex<f32>, Complex<f32>) {
    let (a, a_sq) = fortescue_ops();
    let zero = (pa + pb + pc) / 3.0;
    let pos = (pa + a * pb + a_sq * pc) / 3.0;
    let neg = (pa + a_sq * pb + a * pc) / 3.0;
    (zero, pos, neg)
}

/// Computes voltage unbalance via Fortescue (§5.7).
///
/// # Arguments
///
/// * `v_rms` - RMS voltage of each phase (L1, L2, L3) in volts.
/// * `v_angles_deg` - Phase angle of each phase voltage in degrees.
///
/// # Returns
///
/// An `UnbalanceMetrics` struct populated with the voltage symmetrical components and ratios.
pub fn calculate_voltage_unbalance(v_rms: &[f32; 3], v_angles_deg: &[f32; 3]) -> UnbalanceMetrics {
    if v_rms[0] <= 0.0 && v_rms[1] <= 0.0 && v_rms[2] <= 0.0 {
        return UnbalanceMetrics::default();
    }

    let v_a = Complex::from_polar(v_rms[0], v_angles_deg[0].to_radians());
    let v_b = Complex::from_polar(v_rms[1], v_angles_deg[1].to_radians());
    let v_c = Complex::from_polar(v_rms[2], v_angles_deg[2].to_radians());

    let (v0, v1, v2) = symmetrical_components(v_a, v_b, v_c);
    let v1_mag = v1.norm();

    let (u2_pct, u0_pct) = if v1_mag > 1e-4 {
        ((v2.norm() / v1_mag) * 100.0, (v0.norm() / v1_mag) * 100.0)
    } else {
        (0.0, 0.0)
    };

    UnbalanceMetrics {
        v0_zero_seq: v0.norm(),
        v1_pos_seq: v1_mag,
        v2_neg_seq: v2.norm(),
        u2_neg_ratio_pct: u2_pct.clamp(0.0, 100.0),
        u0_zero_ratio_pct: u0_pct.clamp(0.0, 100.0),
        ..Default::default()
    }
}

/// Computes current unbalance via Fortescue (§5.13.6).
///
/// # Arguments
///
/// * `i_rms` - RMS current of each phase (L1, L2, L3) in amperes.
/// * `i_angles_deg` - Phase angle of each phase current in degrees.
///
/// # Returns
///
/// An `UnbalanceMetrics` struct populated with the current symmetrical components and ratios.
pub fn calculate_current_unbalance(i_rms: &[f32; 3], i_angles_deg: &[f32; 3]) -> UnbalanceMetrics {
    if i_rms[0] <= 0.0 && i_rms[1] <= 0.0 && i_rms[2] <= 0.0 {
        return UnbalanceMetrics::default();
    }

    let i_a = Complex::from_polar(i_rms[0], i_angles_deg[0].to_radians());
    let i_b = Complex::from_polar(i_rms[1], i_angles_deg[1].to_radians());
    let i_c = Complex::from_polar(i_rms[2], i_angles_deg[2].to_radians());

    let (i0, i1, i2) = symmetrical_components(i_a, i_b, i_c);
    let i1_mag = i1.norm();

    let (u2_pct, u0_pct) = if i1_mag > 1e-4 {
        ((i2.norm() / i1_mag) * 100.0, (i0.norm() / i1_mag) * 100.0)
    } else {
        (0.0, 0.0)
    };

    UnbalanceMetrics {
        i0_zero_seq: i0.norm(),
        i1_pos_seq: i1_mag,
        i2_neg_seq: i2.norm(),
        u2_i_ratio_pct: u2_pct.clamp(0.0, 100.0),
        u0_i_ratio_pct: u0_pct.clamp(0.0, 100.0),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that a perfectly balanced 230 V system yields zero unbalance metrics.
    #[test]
    fn test_perfectly_balanced_system() {
        // Balanced 230V system with 120-degree phase shifts
        let v_rms = [230.0, 230.0, 230.0];
        let v_angles = [0.0, -120.0, 120.0];

        let metrics = calculate_voltage_unbalance(&v_rms, &v_angles);

        assert!((metrics.v1_pos_seq - 230.0).abs() < 1e-2);
        assert!(metrics.v2_neg_seq < 1e-2);
        assert!(metrics.v0_zero_seq < 1e-2);
        assert!(metrics.u2_neg_ratio_pct < 1e-2);
        assert!(metrics.u0_zero_ratio_pct < 1e-2);
    }

    /// Verifies that an unbalanced system yields non-zero negative-sequence unbalance metrics.
    #[test]
    fn test_unbalanced_system() {
        // Unbalanced magnitudes
        let v_rms = [230.0, 200.0, 240.0];
        let v_angles = [0.0, -120.0, 120.0];

        let metrics = calculate_voltage_unbalance(&v_rms, &v_angles);

        assert!(metrics.v1_pos_seq > 200.0);
        assert!(metrics.v2_neg_seq > 5.0);
        assert!(metrics.u2_neg_ratio_pct > 0.0);
    }
}
