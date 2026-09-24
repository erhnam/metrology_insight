//! IEC 62053-21 Table 3 error-limit compliance tests.
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

use metrology_insight::accuracy_test::{
    run_accuracy_test, run_polyphase_accuracy_test, PhaseTestPoint,
};

const UN_V: f32 = 230.0;
const IN_A: f32 = 5.0;
const IMAX_A: f32 = 10.0;
const MIN_CYCLES: u32 = 1000; // 20 s @ 50 Hz per IEC 62053-21 recommendation

/// Executes accuracy test and verifies error is strictly within Class 1 limits.
fn assert_test_point(i_rms: f32, pf: f32, limit_pct: f64, label: &str) {
    let r = run_accuracy_test(UN_V, i_rms, pf, 50.0, MIN_CYCLES);
    assert!(
        r.error_pct.abs() < limit_pct,
        "FAIL [{}]: error = {:.3}%, limit = ±{:.1}%",
        label,
        r.error_pct,
        limit_pct
    );
}

#[test]
fn point1_005ln_pf1() {
    assert_test_point(0.05 * IN_A, 1.0, 1.5, "0.05 In, PF=1.0");
}

#[test]
fn point2_01ln_pf1() {
    assert_test_point(0.1 * IN_A, 1.0, 1.0, "0.1 In, PF=1.0");
}

#[test]
fn point3_05ln_pf1() {
    assert_test_point(0.5 * IN_A, 1.0, 1.0, "0.5 In, PF=1.0");
}

#[test]
fn point4_in_pf1() {
    assert_test_point(IN_A, 1.0, 1.0, "In, PF=1.0");
}

#[test]
fn point5_imax_pf1() {
    assert_test_point(IMAX_A, 1.0, 1.0, "Imax, PF=1.0");
}

#[test]
fn point6_01ln_pf05_ind() {
    assert_test_point(0.1 * IN_A, 0.5, 1.5, "0.1 In, PF=0.5 ind");
}

#[test]
fn point7_02ln_pf05_ind() {
    assert_test_point(0.2 * IN_A, 0.5, 1.0, "0.2 In, PF=0.5 ind");
}

#[test]
fn point8_05ln_pf05_ind() {
    assert_test_point(0.5 * IN_A, 0.5, 1.0, "0.5 In, PF=0.5 ind");
}

#[test]
fn point9_in_pf05_ind() {
    assert_test_point(IN_A, 0.5, 1.0, "In, PF=0.5 ind");
}

#[test]
fn point10_05ln_pf08_cap() {
    assert_test_point(0.5 * IN_A, 0.8, 1.0, "0.5 In, PF=0.8 cap");
}

#[test]
fn point11_in_pf08_cap() {
    assert_test_point(IN_A, 0.8, 1.0, "In, PF=0.8 cap");
}

// Point 12: Polyphase meter with unbalanced load — apply current to only one phase.
/// Checks the polyphase error with only one phase loaded against the 2.0%
/// Class 1 limit.
#[test]
fn point12_unbalanced_load() {
    let loaded = PhaseTestPoint {
        v_rms: UN_V,
        i_rms: IN_A,
        pf: 1.0,
    };
    let unloaded = PhaseTestPoint {
        v_rms: UN_V,
        i_rms: 0.0,
        pf: 1.0,
    };
    let r = run_polyphase_accuracy_test([loaded, unloaded, unloaded], 50.0, 1000);
    assert!(
        r.error_pct.abs() < 2.0,
        "FAIL [Unbalanced (1 phase loaded), PF=1.0]: error = {:.3}%, limit = ±2.0%",
        r.error_pct
    );
}
