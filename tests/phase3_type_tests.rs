//! Type-test benchmarks: measurement uncertainty and limits (§7.3).
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

use metrology_insight::accuracy_test::{generate_cycle, run_accuracy_test};

const UN_V: f32 = 230.0;
const IN_A: f32 = 5.0;
const FN_HZ: f32 = 50.0;
const MIN_CYCLES: u32 = 1000;

// ─── 3.1 Measurement uncertainty (§7.3) ───────────────────────────────────────

/// Verifies the expanded measurement uncertainty (k=2) meets the Class 1
/// limit of 0.33% using Type A (repeated runs) and Type B (component
/// tolerance) uncertainty estimation.
///
/// # Panics
///
/// Panics if the expanded uncertainty is not below 0.33%.
#[test]
fn uncertainty_type_a() {
    let n = 10;
    let mut errors = Vec::with_capacity(n);
    for _ in 0..n {
        let r = run_accuracy_test(UN_V, IN_A, 1.0, FN_HZ, MIN_CYCLES);
        errors.push(r.error_pct);
    }
    let mean = errors.iter().copied().sum::<f64>() / n as f64;
    let variance = errors.iter().map(|&e| (e - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
    let std_dev = variance.sqrt();

    // Type A standard uncertainty u_A = std_dev / sqrt(n)
    let ua = std_dev / (n as f64).sqrt();

    // Type B estimate: ADC resolution (24-bit ±1.2V → ~0.07 µV/LSB),
    // voltage divider tolerance (±1%), CT ratio tolerance (±1%).
    // Conservative RSS: sqrt(0.01^2 + 0.01^2) ≈ 0.014 = 1.4%
    let ub = 0.014;

    // Combined standard uncertainty
    let uc = (ua * ua + ub * ub).sqrt();

    // Expanded uncertainty (k=2, 95% confidence)
    let u_expanded = uc * 2.0;

    // For Class 1, expanded uncertainty must be ≤ 1/3 of limit = 0.33%
    assert!(
        u_expanded < 0.33,
        "Expanded uncertainty too high: {:.4}% (limit 0.33%) — u_A={:.6}%, u_B={:.4}%",
        u_expanded,
        ua,
        ub
    );
}

// ─── 3.2 Meter constant (§7.4) ────────────────────────────────────────────────
//
// The meter constant defines the relationship between accumulated internal energy
// and output pulses (digital or LED).
//
// Internal resolution: energy accumulated in µJ (i128), converted to kWh via:
//   factor = JOULES_TO_KWH / W_SEC_TO_UJ  (see energy.rs)
//   = (1/3_600_000) / 1_000_000 = 1/3.6e12
//
// Proposed constant: 1 pulse per Wh (1000 pulses/kWh).
// At In=5A, Un=230V, PF=1.0: P = 1150 W → 1 pulse every ~3.13 seconds.
// The i128 accumulator overflows at 2^127 µJ ≈ 1.7e31 µJ ≈ 4.7e18 kWh —
// effectively never in the device lifetime.

/// Verifies the energy accumulator scaling factor by checking that the error
/// between measured and reference energy is near zero.
///
/// # Panics
///
/// Panics if the measured energy deviates from the reference by more than 1%.
#[test]
fn meter_constant_scaling() {
    // Verify that the energy accumulator scales correctly by measuring
    // known energy and checking the ratio.
    let r = run_accuracy_test(UN_V, IN_A, 1.0, FN_HZ, MIN_CYCLES);

    // Ratio measured/reference should be near 1.0 (error near 0%)
    assert!(
        r.error_pct.abs() < 1.0,
        "Energy scaling error: {:.4}% — check energy.rs conversion factor",
        r.error_pct
    );
}

// ─── 3.4 No-load condition (§7.6) ─────────────────────────────────────────────

/// Verifies that no energy is accumulated under no-load conditions
/// (reference voltage applied, zero current).
///
/// # Panics
///
/// Panics if any energy is accumulated with zero load current.
#[test]
fn no_load_zero_energy() {
    let v_test = 1.15 * UN_V; // 264.5 V
    let i_zero = 0.0;
    let cycles = 30000; // ~10 minutes @ 50 Hz

    let r = run_accuracy_test(v_test, i_zero, 1.0, FN_HZ, cycles);
    assert!(
        r.energy_meas_wh.abs() < 1e-9,
        "No-load test FAIL: accumulated {:.12} Wh with I=0 (limit: 0 Wh)",
        r.energy_meas_wh
    );
}

/// Verifies that a current below the noise gate (4 mA) accumulates no energy.
///
/// # Panics
///
/// Panics if energy is accumulated when the current is below the noise gate.
#[test]
fn noise_below_threshold_no_energy() {
    // Current well below noise gate (ist_a * 0.4 = 4 mA) should not
    // accumulate energy. Noise gate uses config.standard_values.ist_a * 0.4.
    let i_noise = 0.001; // 1 mA (below 4 mA gate threshold)
    let cycles = 30000; // ~10 minutes

    let r = run_accuracy_test(UN_V, i_noise, 1.0, FN_HZ, cycles);
    assert!(
        r.energy_meas_wh.abs() < 1e-9,
        "Noise threshold FAIL: accumulated {:.10} Wh at I={:.4} A (limit: 0 Wh)",
        r.energy_meas_wh,
        i_noise
    );
}

// ─── 3.5 Starting current (§7.7) ──────────────────────────────────────────────

/// Returns the starting current Ist for a Class 1 CT-connected meter.
///
/// # Returns
///
/// The starting current (10 mA) as 0.2% of the reference current In.
fn ist() -> f32 {
    0.002 * IN_A // 10 mA for CT connection (Class 1)
}

/// Returns the minimum current Imin for a Class 1 CT-connected meter.
///
/// # Returns
///
/// The minimum current (0.10 A) as 2% of the reference current In.
fn imin() -> f32 {
    0.02 * IN_A // 0.10 A for CT connection (Class 1)
}

/// Verifies that the meter registers energy at the starting current Ist.
///
/// # Panics
///
/// Panics if no energy is registered at the starting current.
#[test]
fn starting_current_registers() {
    let r = run_accuracy_test(UN_V, ist(), 1.0, FN_HZ, MIN_CYCLES);
    assert!(
        r.energy_meas_wh > 0.0,
        "Starting current FAIL: no energy registered at I=Ist={:.4} A",
        ist()
    );
}

/// Verifies that operation slightly below the starting current produces a
/// reasonably small error rather than anomalous energy registration.
///
/// # Panics
///
/// Panics if the error at 0.9×Ist exceeds 10%.
#[test]
fn below_starting_current_reasonable() {
    let i_below = 0.9 * ist(); // 9 mA
    let r = run_accuracy_test(UN_V, i_below, 1.0, FN_HZ, MIN_CYCLES);
    // The standard does not require zero energy below Ist.
    // Verify the error is within reasonable bounds (not anomalously large).
    assert!(
        r.error_pct.abs() < 10.0,
        "Below starting current: error too large ({:.4}%) at I={:.4} A (0.9×Ist)",
        r.error_pct,
        i_below
    );
}

// ─── 3.6 Repeatability (§7.8) ─────────────────────────────────────────────────

/// Verifies measurement repeatability across 10 identical runs, requiring a
/// standard deviation below 0.2%.
///
/// # Panics
///
/// Panics if the standard deviation of the 10 runs is not below 0.2%.
#[test]
fn repeatability_10_measurements() {
    let n = 10;
    let mut errors = Vec::with_capacity(n);
    for _ in 0..n {
        let r = run_accuracy_test(UN_V, IN_A, 1.0, FN_HZ, 500);
        errors.push(r.error_pct);
    }
    let mean = errors.iter().copied().sum::<f64>() / n as f64;
    let variance = errors.iter().map(|&e| (e - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
    let std_dev = variance.sqrt();

    assert!(
        std_dev < 0.2,
        "Repeatability too high: std_dev = {:.4}% (limit 0.2%)",
        std_dev
    );
}

// ─── 3.7 Rapid current variations (§9.4.12) ───────────────────────────────────

/// Runs a rapid current step test (low current then high current) and returns
/// the percent error of measured energy versus the reference.
///
/// # Arguments
///
/// * `low_a` - RMS current of the low-current phase.
/// * `high_a` - RMS current of the high-current phase (abrupt step).
/// * `cycles_low` - Number of cycles applied at the low current.
/// * `cycles_high` - Number of cycles applied at the high current.
///
/// # Returns
///
/// The percent error between measured and reference energy over the whole run.
fn run_step_test(low_a: f32, high_a: f32, cycles_low: u32, cycles_high: u32) -> f64 {
    let fs = 8000.0;
    let cfg = metrology_insight::MetrologyInsightConfig {
        adc_samples_seconds: fs,
        adc_samples_per_cycle: (fs / FN_HZ) as f64,
        nominal_freq: FN_HZ,
        ..Default::default()
    };
    let mut insight = metrology_insight::MetrologyInsight::new(cfg);

    let dt_s = 1.0 / FN_HZ;

    // Low current phase
    for _ in 0..cycles_low {
        let (v, i) = generate_cycle(UN_V, low_a, 1.0, FN_HZ, fs);
        let n = v.len().min(metrology_insight::MAX_SIGNAL_SAMPLES);
        insight.socket.phases[0].voltage.real_wave[..n].copy_from_slice(&v[..n]);
        insight.socket.phases[0].voltage.real_wave_len = n;
        insight.socket.phases[0].current.real_wave[..n].copy_from_slice(&i[..n]);
        insight.socket.phases[0].current.real_wave_len = n;
        insight.process_and_update_metrics(1);
        insight.socket.phases[0].voltage.clear_samples();
        insight.socket.phases[0].current.clear_samples();
    }

    // High current phase (abrupt step)
    for _ in 0..cycles_high {
        let (v, i) = generate_cycle(UN_V, high_a, 1.0, FN_HZ, fs);
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

    // Reference: average power weighted by duration
    let p_low = (UN_V as f64) * (low_a as f64) * 1.0;
    let p_high = (UN_V as f64) * (high_a as f64) * 1.0;
    let ref_wh = (p_low * cycles_low as f64 + p_high * cycles_high as f64) * dt_s as f64 / 3600.0;

    if ref_wh.abs() > 1e-12 {
        (imported - ref_wh) / ref_wh * 100.0
    } else {
        0.0
    }
}

/// Verifies accuracy when the current steps abruptly from Imin to Imax.
///
/// # Panics
///
/// Panics if the step error exceeds 2%.
#[test]
fn step_imin_to_imax() {
    let i_low = imin(); // 0.10 A (CT)
    let imax = 2.0 * IN_A; // 10 A
    let err = run_step_test(i_low, imax, 50, 50);
    assert!(err.abs() < 2.0, "Step Imin→Imax error: {:.4}%", err);
}

/// Verifies accuracy when the current steps abruptly from Imax to Imin.
///
/// # Panics
///
/// Panics if the step error exceeds 2%.
#[test]
fn step_imax_to_imin() {
    let i_low = imin(); // 0.10 A (CT)
    let imax = 2.0 * IN_A; // 10 A
    let err = run_step_test(imax, i_low, 50, 50);
    assert!(err.abs() < 2.0, "Step Imax→Imin error: {:.4}%", err);
}
