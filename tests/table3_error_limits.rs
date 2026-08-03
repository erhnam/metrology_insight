//! IEC 62053-21 Table 3 error-limit compliance tests.
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

use metrology_insight::accuracy_test::{run_accuracy_test, run_polyphase_accuracy_test, PhaseTestPoint};

const UN_V: f32 = 230.0;
const IN_A: f32 = 5.0;
const IMAX_A: f32 = 10.0;
const MIN_CYCLES: u32 = 1000; // 20 s @ 50 Hz per IEC 62053-21 recommendation

/// Asserts that the given percent error is within the allowed limit.
///
/// # Arguments
///
/// * `error_pct` - Measured percent error (signed).
/// * `limit_pct` - Maximum allowed absolute percent error.
/// * `label` - Test point description used in the failure message.
///
/// # Panics
///
/// Panics if the absolute error is not below the limit.
fn check(error_pct: f64, limit_pct: f64, label: &str) {
    assert!(
        error_pct.abs() < limit_pct,
        "FAIL [{}]: error = {:.3}%, limit = ±{:.1}%",
        label,
        error_pct,
        limit_pct
    );
}

/// Checks the error at 0.05 In with PF=1.0 against the 1.5% Class 1 limit.
///
/// # Panics
///
/// Panics if the error is not below the limit.
#[test]
fn point1_005ln_pf1() {
    let r = run_accuracy_test(UN_V, 0.05 * IN_A, 1.0, 50.0, MIN_CYCLES);
    check(r.error_pct, 1.5, "0.05 In, PF=1.0");
}

/// Checks the error at 0.1 In with PF=1.0 against the 1.0% Class 1 limit.
///
/// # Panics
///
/// Panics if the error is not below the limit.
#[test]
fn point2_01ln_pf1() {
    let r = run_accuracy_test(UN_V, 0.1 * IN_A, 1.0, 50.0, MIN_CYCLES);
    check(r.error_pct, 1.0, "0.1 In, PF=1.0");
}

/// Checks the error at 0.5 In with PF=1.0 against the 1.0% Class 1 limit.
///
/// # Panics
///
/// Panics if the error is not below the limit.
#[test]
fn point3_05ln_pf1() {
    let r = run_accuracy_test(UN_V, 0.5 * IN_A, 1.0, 50.0, MIN_CYCLES);
    check(r.error_pct, 1.0, "0.5 In, PF=1.0");
}

/// Checks the error at In with PF=1.0 against the 1.0% Class 1 limit.
///
/// # Panics
///
/// Panics if the error is not below the limit.
#[test]
fn point4_in_pf1() {
    let r = run_accuracy_test(UN_V, IN_A, 1.0, 50.0, MIN_CYCLES);
    check(r.error_pct, 1.0, "In, PF=1.0");
}

/// Checks the error at Imax with PF=1.0 against the 1.0% Class 1 limit.
///
/// # Panics
///
/// Panics if the error is not below the limit.
#[test]
fn point5_imax_pf1() {
    let r = run_accuracy_test(UN_V, IMAX_A, 1.0, 50.0, MIN_CYCLES);
    check(r.error_pct, 1.0, "Imax, PF=1.0");
}

/// Checks the error at 0.1 In with PF=0.5 inductive against the 1.5% Class 1
/// limit.
///
/// # Panics
///
/// Panics if the error is not below the limit.
#[test]
fn point6_01ln_pf05_ind() {
    let r = run_accuracy_test(UN_V, 0.1 * IN_A, 0.5, 50.0, MIN_CYCLES);
    check(r.error_pct, 1.5, "0.1 In, PF=0.5 ind");
}

/// Checks the error at 0.2 In with PF=0.5 inductive against the 1.0% Class 1
/// limit.
///
/// # Panics
///
/// Panics if the error is not below the limit.
#[test]
fn point7_02ln_pf05_ind() {
    let r = run_accuracy_test(UN_V, 0.2 * IN_A, 0.5, 50.0, MIN_CYCLES);
    check(r.error_pct, 1.0, "0.2 In, PF=0.5 ind");
}

/// Checks the error at 0.5 In with PF=0.5 inductive against the 1.0% Class 1
/// limit.
///
/// # Panics
///
/// Panics if the error is not below the limit.
#[test]
fn point8_05ln_pf05_ind() {
    let r = run_accuracy_test(UN_V, 0.5 * IN_A, 0.5, 50.0, MIN_CYCLES);
    check(r.error_pct, 1.0, "0.5 In, PF=0.5 ind");
}

/// Checks the error at In with PF=0.5 inductive against the 1.0% Class 1 limit.
///
/// # Panics
///
/// Panics if the error is not below the limit.
#[test]
fn point9_in_pf05_ind() {
    let r = run_accuracy_test(UN_V, IN_A, 0.5, 50.0, MIN_CYCLES);
    check(r.error_pct, 1.0, "In, PF=0.5 ind");
}

/// Checks the error at 0.5 In with PF=0.8 capacitive against the 1.0% Class 1
/// limit.
///
/// # Panics
///
/// Panics if the error is not below the limit.
#[test]
fn point10_05ln_pf08_cap() {
    let r = run_accuracy_test(UN_V, 0.5 * IN_A, 0.8, 50.0, MIN_CYCLES);
    check(r.error_pct, 1.0, "0.5 In, PF=0.8 cap");
}

/// Checks the error at In with PF=0.8 capacitive against the 1.0% Class 1
/// limit.
///
/// # Panics
///
/// Panics if the error is not below the limit.
#[test]
fn point11_in_pf08_cap() {
    let r = run_accuracy_test(UN_V, IN_A, 0.8, 50.0, MIN_CYCLES);
    check(r.error_pct, 1.0, "In, PF=0.8 cap");
}

// Point 12: Polyphase meter with unbalanced load — apply current to only one phase.
/// Checks the polyphase error with only one phase loaded against the 2.0%
/// Class 1 limit.
///
/// # Panics
///
/// Panics if the error is not below the limit.
#[test]
fn point12_unbalanced_load() {
    let loaded = PhaseTestPoint { v_rms: UN_V, i_rms: IN_A, pf: 1.0 };
    let unloaded = PhaseTestPoint { v_rms: UN_V, i_rms: 0.0, pf: 1.0 };
    let r = run_polyphase_accuracy_test([loaded, unloaded, unloaded], 50.0, 1000);
    check(r.error_pct, 2.0, "Unbalanced (1 phase loaded), PF=1.0");
}
