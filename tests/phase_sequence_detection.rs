// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

//! Phase sequence detection tests (Phase 2.6).
//!
//! Verifies that the symmetrical components (Fortescue) calculation correctly
//! identifies the phase sequence (positive L1-L2-L3 vs negative L1-L3-L2).

use num_complex::Complex;

/// Fortescue factor a = e^(j·120°).
///
/// # Returns
///
/// The complex operator a = e^(j·120°).
fn a() -> Complex<f32> {
    Complex::from_polar(1.0, (120.0_f32).to_radians())
}

/// Computes the symmetrical components from three phasors.
///
/// # Arguments
///
/// * `pa` - Phasor of phase A.
/// * `pb` - Phasor of phase B.
/// * `pc` - Phasor of phase C.
///
/// # Returns
///
/// A tuple with the zero, positive, and negative sequence components.
fn symmetrical_components(
    pa: Complex<f32>,
    pb: Complex<f32>,
    pc: Complex<f32>,
) -> (Complex<f32>, Complex<f32>, Complex<f32>) {
    let a_val = a();
    let a_sq = a_val * a_val;
    let zero = (pa + pb + pc) / 3.0;
    let pos = (pa + a_val * pb + a_sq * pc) / 3.0;
    let neg = (pa + a_sq * pb + a_val * pc) / 3.0;
    (zero, pos, neg)
}

/// Creates a phasor from magnitude and angle.
///
/// # Arguments
///
/// * `mag` - Magnitude of the phasor.
/// * `angle_rad` - Phase angle in radians.
///
/// # Returns
///
/// The complex phasor with the given magnitude and angle.
fn phasor(mag: f32, angle_rad: f32) -> Complex<f32> {
    Complex::from_polar(mag, angle_rad)
}

/// Verifies that a balanced positive (direct) phase sequence yields a dominant
/// positive component of ~230 V and a negligible negative component.
///
/// # Panics
///
/// Panics if the positive magnitude deviates from 230 V or the negative
/// magnitude is not negligible.
#[test]
fn test_positive_sequence_dominant() {
    // Positive sequence: L1=0°, L2=-120°, L3=+120°
    let v1 = phasor(230.0, 0.0);
    let v2 = phasor(230.0, -120.0_f32.to_radians());
    let v3 = phasor(230.0, 120.0_f32.to_radians());

    let (_zero, pos, neg) = symmetrical_components(v1, v2, v3);

    // In a balanced positive sequence: positive = 230 V, negative ≈ 0
    assert!(
        (pos.norm() - 230.0).abs() < 0.1,
        "Positive seq magnitude {:.2} V ≠ 230 V",
        pos.norm()
    );
    assert!(
        neg.norm() < 0.1,
        "Negative seq magnitude {:.4} V should be ~0",
        neg.norm()
    );
}

/// Verifies that a balanced negative (inverse) phase sequence yields a dominant
/// negative component of ~230 V and a negligible positive component.
///
/// # Panics
///
/// Panics if the negative magnitude deviates from 230 V or the positive
/// magnitude is not negligible.
#[test]
fn test_negative_sequence_dominant() {
    // Negative sequence: L1=0°, L2=+120°, L3=-120° (L2↔L3 swapped)
    let v1 = phasor(230.0, 0.0);
    let v2 = phasor(230.0, 120.0_f32.to_radians());
    let v3 = phasor(230.0, -120.0_f32.to_radians());

    let (_zero, pos, neg) = symmetrical_components(v1, v2, v3);

    // In a balanced negative sequence: positive ≈ 0, negative = 230 V
    assert!(
        pos.norm() < 0.1,
        "Positive seq magnitude {:.4} V should be ~0",
        pos.norm()
    );
    assert!(
        (neg.norm() - 230.0).abs() < 0.1,
        "Negative seq magnitude {:.2} V ≠ 230 V",
        neg.norm()
    );
}

/// Verifies that three in-phase phasors yield a dominant zero-sequence
/// component of ~230 V with negligible positive and negative components.
///
/// # Panics
///
/// Panics if the zero magnitude deviates from 230 V or the positive/negative
/// magnitudes are not negligible.
#[test]
fn test_zero_sequence_dominant() {
    // Zero sequence: all phases in phase (L1=L2=L3)
    let v1 = phasor(230.0, 0.0);
    let v2 = phasor(230.0, 0.0);
    let v3 = phasor(230.0, 0.0);

    let (zero, pos, neg) = symmetrical_components(v1, v2, v3);

    assert!(
        (zero.norm() - 230.0).abs() < 0.1,
        "Zero seq magnitude {:.2} V ≠ 230 V",
        zero.norm()
    );
    assert!(
        pos.norm() < 0.1,
        "Positive seq magnitude {:.4} V should be ~0",
        pos.norm()
    );
    assert!(
        neg.norm() < 0.1,
        "Negative seq magnitude {:.4} V should be ~0",
        neg.norm()
    );
}

/// Verifies that for an unbalanced load the positive component still dominates
/// and stays above 200 V.
///
/// # Panics
///
/// Panics if the positive component does not dominate the negative one or is
/// below 200 V.
#[test]
fn test_unbalanced_sequence_identification() {
    // Unbalanced load: L1=230V/0°, L2=220V/-120°, L3=240V/+120°
    let v1 = phasor(230.0, 0.0);
    let v2 = phasor(220.0, -120.0_f32.to_radians());
    let v3 = phasor(240.0, 120.0_f32.to_radians());

    let (_zero, pos, neg) = symmetrical_components(v1, v2, v3);

    // The positive component must be the dominant one (~230 V)
    // The negative component must be small but non-zero (~10 V with this unbalance)
    assert!(
        pos.norm() > neg.norm(),
        "Positive seq {:.2} V should dominate over negative seq {:.2} V",
        pos.norm(),
        neg.norm()
    );
    assert!(
        pos.norm() > 200.0,
        "Positive seq magnitude {:.2} V unreasonably low",
        pos.norm()
    );
}

/// Verifies that swapping phases L1 and L2 inverts the sequence: the positive
/// component becomes ~0 and the negative component ~230 V.
///
/// # Panics
///
/// Panics if the positive component is not near zero or the negative component
/// deviates from 230 V.
#[test]
fn test_phase_swap_l1_l2() {
    // L1 and L2 swapped: L1=-120°, L2=0°, L3=+120°
    let v1 = phasor(230.0, -120.0_f32.to_radians());
    let v2 = phasor(230.0, 0.0);
    let v3 = phasor(230.0, 120.0_f32.to_radians());

    let (_zero, pos, neg) = symmetrical_components(v1, v2, v3);

    // With L1↔L2, the sequence remains positive (the cyclic order L1-L2-L3
    // is now -120°, 0°, 120°, which is still a positive sequence — just rotated).
    // In fact, swapping L1 and L2 inverts the sequence.
    // Original: 0°, -120°, 120° → positive
    // Swap L1↔L2: -120°, 0°, 120° → let's try:
    // a²·pc: e^(j·240°) · 230·e^(j·120°) = 230·e^(j·360°) = 230
    // a·pb: e^(j·120°) · 230·e^(j·0°) = 230·e^(j·120°)
    // With the formula: pos = (pa + a·pb + a²·pc)/3
    // = (230·e^(-j·120°) + 230·e^(j·120°) + 230·e^(j·360°))/3
    // = (230·e^(-j·120°) + 230·e^(j·120°) + 230)/3
    // = (230·(-1/2 - j·√3/2) + 230·(-1/2 + j·√3/2) + 230)/3
    // = (-115 - j·199.2 - 115 + j·199.2 + 230)/3 = 0/3 = 0
    // The positive should be ~0 and the negative ~230 V

    assert!(
        pos.norm() < 1.0,
        "Positive seq {:.4} V should be ~0 after L1↔L2 swap",
        pos.norm()
    );
    assert!(
        (neg.norm() - 230.0).abs() < 1.0,
        "Negative seq {:.2} V should be ~230 V after L1↔L2 swap",
        neg.norm()
    );
}
