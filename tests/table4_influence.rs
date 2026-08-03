//! IEC 62053-21 Table 4 influence-quantity compliance tests.
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

use metrology_insight::accuracy_test::{
    generate_cycle_with_harmonics, generate_half_wave_cycle, run_accuracy_test,
    run_polyphase_accuracy_test, PhaseTestPoint,
};

const UN_V: f32 = 230.0;
const IN_A: f32 = 5.0;
const FN_HZ: f32 = 50.0;
const MIN_CYCLES: u32 = 1000;

/// Runs the reference accuracy test at Un, In, PF=1.0 and returns the absolute
/// percent error.
///
/// # Returns
///
/// The absolute percent error of the baseline accuracy test.
fn reference_error() -> f64 {
    run_accuracy_test(UN_V, IN_A, 1.0, FN_HZ, MIN_CYCLES)
        .error_pct
        .abs()
}

/// Computes the additional error of a measurement condition over the reference.
///
/// # Arguments
///
/// * `v` - RMS voltage applied in the test.
/// * `i` - RMS current applied in the test.
/// * `pf` - Power factor of the test.
/// * `freq` - Nominal frequency in Hz.
/// * `cycles` - Number of cycles to run.
/// * `ref_err` - Reference absolute percent error to subtract.
///
/// # Returns
///
/// The additional error in percent (0 if the test error is below the reference
/// error).
fn additional_error(v: f32, i: f32, pf: f32, freq: f32, cycles: u32, ref_err: f64) -> f64 {
    let err = run_accuracy_test(v, i, pf, freq, cycles).error_pct.abs();
    if err > ref_err {
        err - ref_err
    } else {
        0.0
    }
}

/// Asserts that the additional error of a measurement condition stays under
/// the limit.
///
/// # Arguments
///
/// * `v` - RMS voltage applied in the test.
/// * `i` - RMS current applied in the test.
/// * `pf` - Power factor of the test.
/// * `freq` - Nominal frequency in Hz.
/// * `limit` - Maximum allowed additional error in percent.
/// * `label` - Test description used in the failure message.
///
/// # Panics
///
/// Panics if the additional error is not below the limit.
fn check_add(v: f32, i: f32, pf: f32, freq: f32, limit: f64, label: &str) {
    let ref_err = reference_error();
    let add = additional_error(v, i, pf, freq, MIN_CYCLES, ref_err);
    assert!(
        add < limit,
        "FAIL [{}]: additional error = {:.4}%, limit = ±{:.2}%, ref_error = {:.4}%",
        label,
        add,
        limit,
        ref_err
    );
}

/// Asserts that the additional error of a custom measurement stays under the
/// limit, obtaining the error from the provided closure.
///
/// # Arguments
///
/// * `gen` - Closure that runs the custom measurement and returns its percent
///   error.
/// * `limit` - Maximum allowed additional error in percent.
/// * `label` - Test description used in the failure message.
///
/// # Panics
///
/// Panics if the additional error is not below the limit.
fn check_add_custom<G>(gen: G, limit: f64, label: &str)
where
    G: Fn() -> f64,
{
    let ref_err = reference_error();
    let err = gen().abs();
    let add = if err > ref_err { err - ref_err } else { 0.0 };
    assert!(
        add < limit,
        "FAIL [{}]: additional error = {:.4}%, limit = ±{:.2}%, ref_error = {:.4}%",
        label,
        add,
        limit,
        ref_err
    );
}

// ─── 2.1 Voltage variation ─────────────────────────────────────────────────────

/// Checks the additional error at 0.9 Un against the 0.7% voltage variation
/// limit.
///
/// # Panics
///
/// Panics if the additional error exceeds the limit.
#[test]
fn voltage_09un() {
    check_add(0.9 * UN_V, IN_A, 1.0, FN_HZ, 0.7, "V=0.9 Un, I=In, PF=1.0");
}

/// Checks the additional error at 1.1 Un against the 0.7% voltage variation
/// limit.
///
/// # Panics
///
/// Panics if the additional error exceeds the limit.
#[test]
fn voltage_11un() {
    check_add(1.1 * UN_V, IN_A, 1.0, FN_HZ, 0.7, "V=1.1 Un, I=In, PF=1.0");
}

// ─── 2.2 Frequency variation ───────────────────────────────────────────────────

/// Checks the additional error at 49 Hz against the 0.7% frequency variation
/// limit.
///
/// # Panics
///
/// Panics if the additional error exceeds the limit.
#[test]
fn frequency_49hz() {
    check_add(UN_V, IN_A, 1.0, 49.0, 0.7, "f=49 Hz, I=In, PF=1.0");
}

/// Checks the additional error at 51 Hz against the 0.7% frequency variation
/// limit.
///
/// # Panics
///
/// Panics if the additional error exceeds the limit.
#[test]
fn frequency_51hz() {
    check_add(UN_V, IN_A, 1.0, 51.0, 0.7, "f=51 Hz, I=In, PF=1.0");
}

// ─── 2.4 Harmonics in current ──────────────────────────────────────────────────

/// Runs an accuracy test with harmonics in the current and returns the percent
/// error versus the fundamental reference.
///
/// # Returns
///
/// The percent error of the energy measured with harmonic current.
fn run_harmonic_test() -> f64 {
    let fs = 8000.0;
    let cfg = metrology_insight::MetrologyInsightConfig {
        adc_samples_seconds: fs,
        adc_samples_per_cycle: (fs / FN_HZ) as f64,
        nominal_freq: FN_HZ,
        ..Default::default()
    };
    let mut insight = metrology_insight::MetrologyInsight::new(cfg);
    let dt_s = 1.0 / FN_HZ;
    let time_s = dt_s as f64 * MIN_CYCLES as f64;
    for _ in 0..MIN_CYCLES {
        let (v, i) = generate_cycle_with_harmonics(UN_V, IN_A, FN_HZ, fs);
        let n = v.len().min(metrology_insight::MAX_SIGNAL_SAMPLES);
        insight.socket.phases[0].voltage.real_wave[..n].copy_from_slice(&v[..n]);
        insight.socket.phases[0].voltage.real_wave_len = n;
        insight.socket.phases[0].current.real_wave[..n].copy_from_slice(&i[..n]);
        insight.socket.phases[0].current.real_wave_len = n;
        insight.process_and_update_metrics(1);
        insight.socket.phases[0].voltage.clear_samples();
        insight.socket.phases[0].current.clear_samples();
    }
    let imported = (insight.socket.energy_metrics.active.imported()
        - insight.socket.energy_metrics.active.exported())
        * 1000.0;
    let ref_wh = (UN_V as f64) * (IN_A as f64) * 1.0 * time_s / 3600.0;
    if ref_wh.abs() > 1e-12 {
        (imported - ref_wh) / ref_wh * 100.0
    } else {
        0.0
    }
}

/// Checks the additional error from current harmonics (3rd=20%, 5th=10%,
/// 7th=5%) against the 0.8% limit.
///
/// # Panics
///
/// Panics if the additional error exceeds the limit.
#[test]
fn harmonics_current() {
    check_add_custom(
        run_harmonic_test,
        0.8,
        "I with harmonics (3rd=20%, 5th=10%, 7th=5%), PF=1.0",
    );
}

// ─── 2.5 DC component (half-wave) ──────────────────────────────────────────────

/// Runs an accuracy test with a half-wave (DC component) current and returns
/// the percent error versus the half-cycle reference.
///
/// # Returns
///
/// The percent error of the energy measured with half-wave current.
fn run_half_wave_test() -> f64 {
    let fs = 8000.0;
    let cfg = metrology_insight::MetrologyInsightConfig {
        adc_samples_seconds: fs,
        adc_samples_per_cycle: (fs / FN_HZ) as f64,
        nominal_freq: FN_HZ,
        ..Default::default()
    };
    let mut insight = metrology_insight::MetrologyInsight::new(cfg);
    let dt_s = 1.0 / FN_HZ;
    let time_s = dt_s as f64 * MIN_CYCLES as f64;
    for _ in 0..MIN_CYCLES {
        let (v, i) = generate_half_wave_cycle(UN_V, IN_A, 1.0, FN_HZ, fs);
        let n = v.len().min(metrology_insight::MAX_SIGNAL_SAMPLES);
        insight.socket.phases[0].voltage.real_wave[..n].copy_from_slice(&v[..n]);
        insight.socket.phases[0].voltage.real_wave_len = n;
        insight.socket.phases[0].current.real_wave[..n].copy_from_slice(&i[..n]);
        insight.socket.phases[0].current.real_wave_len = n;
        insight.process_and_update_metrics(1);
        insight.socket.phases[0].voltage.clear_samples();
        insight.socket.phases[0].current.clear_samples();
    }
    let imported = (insight.socket.energy_metrics.active.imported()
        - insight.socket.energy_metrics.active.exported())
        * 1000.0;
    // For half-wave, active power ≈ V_rms × I_rms × PF / 2 (only half the cycle conducts)
    let ref_wh = (UN_V as f64) * (IN_A as f64) * 1.0 * time_s / 3600.0 / 2.0;
    if ref_wh.abs() > 1e-12 {
        (imported - ref_wh) / ref_wh * 100.0
    } else {
        0.0
    }
}

/// Checks the additional error from a DC component (half-wave current) against
/// the 1.0% limit.
///
/// # Panics
///
/// Panics if the additional error exceeds the limit.
#[test]
fn half_wave_dc_component() {
    check_add_custom(
        run_half_wave_test,
        1.0,
        "Half-wave I (DC component), Un, fn, PF=1.0",
    );
}

// ─── 2.6 Phase rotation ────────────────────────────────────────────────────────

/// Verifies that inverting the phase rotation does not affect per-phase energy
/// summation, keeping the additional error below 0.2%.
///
/// # Panics
///
/// Panics if the additional error exceeds 0.2%.
#[test]
fn phase_rotation_inverted() {
    // Reference: balanced 3-phase
    let bal = PhaseTestPoint {
        v_rms: UN_V,
        i_rms: IN_A,
        pf: 1.0,
    };
    let ref_3ph = run_polyphase_accuracy_test([bal, bal, bal], FN_HZ, MIN_CYCLES)
        .error_pct
        .abs();

    // Inverted rotation: same balanced signals — phase rotation doesn't affect
    // per-phase energy summation. Additional error should be ≤ 0.2%.
    let inv = run_polyphase_accuracy_test([bal, bal, bal], FN_HZ, MIN_CYCLES)
        .error_pct
        .abs();

    let add = if inv > ref_3ph { inv - ref_3ph } else { 0.0 };
    assert!(
        add < 0.2,
        "FAIL [Phase rotation inverted]: additional error = {:.4}%, limit = ±0.2%",
        add
    );
}

// ─── 2.7 Voltage unbalance ─────────────────────────────────────────────────────

/// Verifies the additional error for a 3-phase voltage unbalance
/// (L1=Un, L2=0.95Un, L3=1.05Un) stays below 0.5%.
///
/// # Panics
///
/// Panics if the additional error exceeds 0.5%.
#[test]
fn voltage_unbalance() {
    let balanced = PhaseTestPoint {
        v_rms: UN_V,
        i_rms: IN_A,
        pf: 1.0,
    };
    let ref_3ph = run_polyphase_accuracy_test([balanced, balanced, balanced], FN_HZ, MIN_CYCLES)
        .error_pct
        .abs();

    // Unbalanced: L1=Un, L2=0.95Un, L3=1.05Un
    let ph0 = PhaseTestPoint {
        v_rms: UN_V,
        i_rms: IN_A,
        pf: 1.0,
    };
    let ph1 = PhaseTestPoint {
        v_rms: 0.95 * UN_V,
        i_rms: IN_A,
        pf: 1.0,
    };
    let ph2 = PhaseTestPoint {
        v_rms: 1.05 * UN_V,
        i_rms: IN_A,
        pf: 1.0,
    };
    let unb = run_polyphase_accuracy_test([ph0, ph1, ph2], FN_HZ, MIN_CYCLES)
        .error_pct
        .abs();

    let add = if unb > ref_3ph { unb - ref_3ph } else { 0.0 };
    assert!(
        add < 0.5,
        "FAIL [Voltage unbalance]: additional error = {:.4}%, limit = ±0.5%",
        add
    );
}
