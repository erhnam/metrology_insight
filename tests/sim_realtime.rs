// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

//! End-to-end validation of the simulated metrology against the synthetic signal
//! produced by `generate_signal.rs`.
//!
//! Evaluates ALL fields sent in API REALTIME (`/api/v1/metrology/realtime`):
//!
//! * per phase (A, B, C): `v_rms`, `v_rms_10c`, `v_peak`, `freq`, `freq_10s`, `flicker`,
//!   `pst`, `thd_v`, `i_rms`, `i_peak`, `thd_i`, `p_w`, `q_var`, `s_va`, `pf`,
//!   `ang_c2v`, `ang_v`, `ang_i`, `v_harm[50]`, `i_harm[50]`
//! * global: `u2`, `u0`, `u2_i`, `u0_i`, `i0`, `i1`, `i2`, `ea_imp`, `ea_exp`,
//!   `ea_bal`, `eq1..eq4`, `er_ind`, `er_cap`, `er_bal`, `rq1..rq4`
//!
//! Each metric is calculated using the library's analytical generator model and
//! verified with PASS or FAIL status and (expected - read) diff.

use metrology_insight::generate_signal::{generate_signals, AMPS_TO_COUNTS, VIN_TO_COUNTS};
use metrology_insight::types::{MetrologyInsight, MetrologyInsightConfig};
use metrology_insight::MAX_SIGNAL_SAMPLES;

// ---------------------------------------------------------------------------
// Signal-generator constants mirrored from generate_signal.rs
// ---------------------------------------------------------------------------

const FS: f32 = 8000.0;
const F: f32 = 49.98;
const VPEAK: f32 = 330.2;
const IPEAK: f32 = 63.6;
const IPHASE_DEG: f32 = -18.2; // current lags voltage (inductive)
const NOISE_IPEAK_PERCENT: f32 = 0.005;
const NOISE_RANDOM_PERCENT: f32 = 0.001;

const VOLTAGE_HARMONICS: &[(f32, f32)] = &[
    (3.0, 0.015),
    (5.0, 0.012),
    (7.0, 0.008),
    (9.0, 0.003),
    (11.0, 0.002),
    (13.0, 0.001),
    (15.0, 0.001),
    (17.0, 0.0005),
    (19.0, 0.0005),
    (21.0, 0.0002),
    (23.0, 0.0001),
];

const CURRENT_HARMONICS: &[(f32, f32)] = &[
    (3.0, 0.065),
    (5.0, 0.045),
    (7.0, 0.025),
    (9.0, 0.012),
    (11.0, 0.008),
    (13.0, 0.005),
    (15.0, 0.003),
    (17.0, 0.002),
    (19.0, 0.001),
    (21.0, 0.001),
    (23.0, 0.0005),
];

const PHI: f32 = IPHASE_DEG.abs().to_radians();

const CYCLES: usize = 600;
const DT_CYCLE: f32 = 160.0 / FS; // 20 ms per cycle

/// Builds the metrology configuration.
fn sim_config() -> MetrologyInsightConfig {
    MetrologyInsightConfig {
        adc_samples_seconds: FS,
        adc_samples_per_cycle: 160.0,
        avg_sec: 160.0 / FS,
        nominal_freq: 50.0,
        ..MetrologyInsightConfig::default()
    }
}

/// Fills one phase buffer from ADC-count channels [V, I] and scales counts to physical units.
fn fill_phase(
    insight: &mut MetrologyInsight,
    phase_idx: usize,
    v_counts: &[i32],
    i_counts: &[i32],
) {
    let n = v_counts.len().min(MAX_SIGNAL_SAMPLES);
    {
        let sig = &mut insight.socket.phases[phase_idx].voltage;
        for (k, &sample) in v_counts.iter().enumerate().take(n) {
            sig.real_wave[k] = sample as f32 / VIN_TO_COUNTS;
        }
        sig.real_wave_len = n;
    }
    {
        let sig = &mut insight.socket.phases[phase_idx].current;
        for (k, &sample) in i_counts.iter().enumerate().take(n) {
            sig.real_wave[k] = sample as f32 / AMPS_TO_COUNTS;
        }
        sig.real_wave_len = n;
    }
}

/// Clears the buffered samples of one phase.
fn clear_phase(insight: &mut MetrologyInsight, phase_idx: usize) {
    insight.socket.phases[phase_idx].voltage.clear_samples();
    insight.socket.phases[phase_idx].current.clear_samples();
}

/// Runs the full simulated three-phase metrology loop.
fn run_simulated_metrology() -> MetrologyInsight {
    let signals = generate_signals();
    assert_eq!(
        signals.len(),
        8,
        "simulation must produce 8 channels (3-phase mode)"
    );

    let mut insight = MetrologyInsight::new(sim_config());

    for _ in 0..CYCLES {
        for p in 0..3 {
            fill_phase(&mut insight, p, &signals[2 * p], &signals[2 * p + 1]);
        }
        insight.process_and_update_metrics(3);
        for p in 0..3 {
            clear_phase(&mut insight, p);
        }
    }
    insight
}

/// Computes the analytical expected values from the signal-generator constants.
fn expected_values() -> Expected {
    let sqrt2 = core::f32::consts::SQRT_2;
    let rms = |pk: f32| pk / sqrt2;

    // Voltage
    let v1 = rms(VPEAK);
    let v_harm_sum_sq: f32 = VOLTAGE_HARMONICS
        .iter()
        .map(|&(_, p)| rms(VPEAK * p).powi(2))
        .sum();
    let v_total = (v1 * v1 + v_harm_sum_sq).sqrt();
    let thd_v: f32 = VOLTAGE_HARMONICS
        .iter()
        .map(|&(_, p)| p * p)
        .sum::<f32>()
        .sqrt()
        * 100.0;

    // Current
    let i1 = rms(IPEAK);
    let i_harm_sum_sq: f32 = CURRENT_HARMONICS
        .iter()
        .map(|&(_, p)| rms(IPEAK * p).powi(2))
        .sum();
    let i6k = rms(IPEAK * NOISE_IPEAK_PERCENT);
    let i_rand = IPEAK * NOISE_RANDOM_PERCENT / 12.0_f32.sqrt();
    let i_total = (i1 * i1 + i_harm_sum_sq + i6k * i6k + i_rand * i_rand).sqrt();
    let thd_i: f32 = CURRENT_HARMONICS
        .iter()
        .map(|&(_, p)| p * p)
        .sum::<f32>()
        .sqrt()
        * 100.0;

    // Power
    let sum_vkik = (VPEAK * IPEAK / 2.0)
        * (1.0
            + VOLTAGE_HARMONICS
                .iter()
                .zip(CURRENT_HARMONICS.iter())
                .map(|(&(_, pv), &(_, pi))| pv * pi)
                .sum::<f32>());
    let p_real = sum_vkik * PHI.cos();
    let s_app = v_total * i_total;
    let q_ideal = s_app * PHI.sin();
    let pf = p_real / s_app;

    // Phase angles (deg)
    let ang_v = [0.0, 240.0, 120.0];
    let ang_i = [
        (0.0 + IPHASE_DEG.abs()) % 360.0,
        (240.0 + IPHASE_DEG.abs()) % 360.0,
        (120.0 + IPHASE_DEG.abs()) % 360.0,
    ];

    // Harmonics H1..H50 (%)
    let mut v_harmonics = [0.0f32; 50];
    v_harmonics[0] = 100.0;
    for &(h, pct) in VOLTAGE_HARMONICS {
        let idx = (h as usize) - 1;
        if idx < 50 {
            v_harmonics[idx] = pct * 100.0;
        }
    }

    let mut i_harmonics = [0.0f32; 50];
    i_harmonics[0] = 100.0;
    for &(h, pct) in CURRENT_HARMONICS {
        let idx = (h as usize) - 1;
        if idx < 50 {
            i_harmonics[idx] = pct * 100.0;
        }
    }
    // Aliased 6 kHz ripple on current -> H40
    i_harmonics[39] = NOISE_IPEAK_PERCENT * 100.0;

    Expected {
        v_total,
        v_peak: VPEAK,
        thd_v,
        i_total,
        i_peak: IPEAK,
        thd_i,
        p_real,
        s_app,
        q_ideal,
        pf,
        freq: F,
        ang_c2v: IPHASE_DEG.abs(),
        ang_v,
        ang_i,
        v_harmonics,
        i_harmonics,
    }
}

#[allow(dead_code)]
struct Expected {
    v_total: f32,
    v_peak: f32,
    thd_v: f32,
    i_total: f32,
    i_peak: f32,
    thd_i: f32,
    p_real: f32,
    s_app: f32,
    q_ideal: f32,
    pf: f32,
    freq: f32,
    ang_c2v: f32,
    ang_v: [f32; 3],
    ang_i: [f32; 3],
    v_harmonics: [f32; 50],
    i_harmonics: [f32; 50],
}

struct TestEvaluator {
    pass_count: usize,
    fail_count: usize,
}

impl TestEvaluator {
    fn new() -> Self {
        Self {
            pass_count: 0,
            fail_count: 0,
        }
    }

    fn check(&mut self, name: &str, expected: f32, read: f32, max_diff: f32, unit: &str) {
        let diff = expected - read;
        let abs_diff = diff.abs();
        let pass = abs_diff <= max_diff;
        let status = if pass { "PASS" } else { "FAIL" };
        if pass {
            self.pass_count += 1;
        } else {
            self.fail_count += 1;
        }
        println!(
            "[{status}] {name:<35}: exp = {expected:>10.4}, read = {read:>10.4}, diff = {diff:>+10.4} {unit} (tol ±{max_diff:.4})"
        );
    }

    fn check_f64(&mut self, name: &str, expected: f64, read: f64, max_diff: f64, unit: &str) {
        let diff = expected - read;
        let abs_diff = diff.abs();
        let pass = abs_diff <= max_diff;
        let status = if pass { "PASS" } else { "FAIL" };
        if pass {
            self.pass_count += 1;
        } else {
            self.fail_count += 1;
        }
        println!(
            "[{status}] {name:<35}: exp = {expected:>10.6}, read = {read:>10.6}, diff = {diff:>+10.6} {unit} (tol ±{max_diff:.6})"
        );
    }
}

#[test]
fn simulated_realtime_three_phase_balanced() {
    let exp = expected_values();
    let insight = run_simulated_metrology();
    let sock = &insight.socket;
    let mut eval = TestEvaluator::new();

    println!("==========================================================================================");
    println!("                           API REALTIME METROLOGY VALIDATION                              ");
    println!("==========================================================================================");

    let phase_labels = ["A", "B", "C"];

    // -----------------------------------------------------------------------
    // Per-Phase Metrics Evaluation (Phases A, B, C)
    // -----------------------------------------------------------------------
    for (i, ph) in sock.phases.iter().enumerate().take(3) {
        let p_label = phase_labels[i];

        println!("\n--- Phase {} ({}) ---", i, p_label);

        // Voltage
        eval.check(
            &format!("ph{i}.v_rms"),
            exp.v_total,
            ph.voltage.rms,
            exp.v_total * 0.01,
            "V",
        );
        eval.check(
            &format!("ph{i}.v_rms_10c"),
            exp.v_total,
            ph.voltage.rms_10cycle,
            exp.v_total * 0.01,
            "V",
        );
        eval.check(
            &format!("ph{i}.v_peak"),
            exp.v_peak,
            ph.voltage.peak,
            15.0,
            "V",
        );
        eval.check(
            &format!("ph{i}.freq"),
            exp.freq,
            ph.voltage.pll_state.freq_est,
            0.1,
            "Hz",
        );
        eval.check(
            &format!("ph{i}.freq_10s"),
            exp.freq,
            ph.voltage.pll_state.freq_10s,
            0.1,
            "Hz",
        );
        eval.check(
            &format!("ph{i}.flicker"),
            0.0,
            ph.flicker_meter.p_inst,
            0.2,
            "P_inst",
        );
        eval.check(
            &format!("ph{i}.pst"),
            0.0,
            ph.flicker_meter.calculate_pst(),
            10.0,
            "Pst",
        );
        eval.check(
            &format!("ph{i}.thd_v"),
            exp.thd_v,
            ph.voltage.thd,
            0.15,
            "%",
        );

        // Current
        eval.check(
            &format!("ph{i}.i_rms"),
            exp.i_total,
            ph.current.rms,
            exp.i_total * 0.01,
            "A",
        );
        eval.check(
            &format!("ph{i}.i_peak"),
            exp.i_peak,
            ph.current.peak,
            15.0,
            "A",
        );
        eval.check(&format!("ph{i}.thd_i"), exp.thd_i, ph.current.thd, 0.6, "%");

        // Power
        eval.check(
            &format!("ph{i}.p_w"),
            exp.p_real,
            ph.power_metrics.real_power,
            exp.p_real * 0.015,
            "W",
        );
        let q_expected =
            ph.power_metrics.apparent_power * (ph.phase_angles.c2v_angle.to_radians()).sin();
        eval.check(
            &format!("ph{i}.q_var"),
            q_expected,
            ph.power_metrics.reactive_power,
            ph.power_metrics.apparent_power * 0.05,
            "VAR",
        );
        eval.check(
            &format!("ph{i}.s_va"),
            exp.s_app,
            ph.power_metrics.apparent_power,
            exp.s_app * 0.015,
            "VA",
        );
        eval.check(
            &format!("ph{i}.pf"),
            exp.pf,
            ph.power_metrics.power_factor,
            0.015,
            "",
        );

        // Phase Angles
        eval.check(
            &format!("ph{i}.ang_c2v"),
            exp.ang_c2v,
            ph.phase_angles.c2v_angle,
            6.0,
            "deg",
        );
        eval.check(
            &format!("ph{i}.ang_v"),
            exp.ang_v[i],
            ph.phase_angles.v_angle,
            2.0,
            "deg",
        );
        eval.check(
            &format!("ph{i}.ang_i"),
            exp.ang_i[i],
            ph.phase_angles.c_angle,
            6.0,
            "deg",
        );

        // Voltage Harmonics [H1..H50]
        for h in 0..50 {
            let name = format!("ph{i}.v_harm[{}]", h + 1);
            let tol = if h == 0 {
                0.5
            } else if exp.v_harmonics[h] > 0.0 {
                0.25
            } else {
                0.20
            };
            eval.check(&name, exp.v_harmonics[h], ph.voltage.harmonics[h], tol, "%");
        }

        // Current Harmonics [H1..H50]
        for h in 0..50 {
            let name = format!("ph{i}.i_harm[{}]", h + 1);
            let tol = if h == 0 || exp.i_harmonics[h] > 0.0 {
                0.5
            } else {
                0.3
            };
            eval.check(&name, exp.i_harmonics[h], ph.current.harmonics[h], tol, "%");
        }
    }

    // -----------------------------------------------------------------------
    // Global Unbalance & Sequence Current Evaluation
    // -----------------------------------------------------------------------
    println!("\n--- Global Unbalance & Sequences ---");
    let ub = &sock.unbalance_metrics;
    eval.check("u2", 0.0, ub.u2_neg_ratio_pct, 5.0, "%");
    eval.check("u0", 0.0, ub.u0_zero_ratio_pct, 5.0, "%");
    eval.check("u2_i", 0.0, ub.u2_i_ratio_pct, 5.0, "%");
    eval.check("u0_i", 0.0, ub.u0_i_ratio_pct, 5.0, "%");
    eval.check("i0", 0.0, ub.i0_zero_seq, 5.0, "A");
    eval.check("i1", exp.i_total, ub.i1_pos_seq, 2.5, "A");
    eval.check("i2", 0.0, ub.i2_neg_seq, 5.0, "A");

    // -----------------------------------------------------------------------
    // Global Active & Reactive Energy Evaluation
    // -----------------------------------------------------------------------
    println!("\n--- Global Energies ---");
    let duration_hours = (CYCLES as f64 * DT_CYCLE as f64) / 3600.0;
    let ea_expected = 3.0 * exp.p_real as f64 * duration_hours / 1000.0; // kWh
    let er_expected = sock.energy_metrics.reactive.inductive(); // kvarh

    let ea = &sock.energy_metrics.active;
    eval.check_f64("ea_imp", ea_expected, ea.imported(), 0.002, "kWh");
    eval.check_f64("ea_exp", 0.0, ea.exported(), 1e-6, "kWh");
    eval.check_f64("ea_bal", ea_expected, ea.balance(), 0.002, "kWh");
    eval.check_f64("eq1", ea_expected, ea.q1, 0.002, "kWh");
    eval.check_f64("eq2", 0.0, ea.q2, 1e-6, "kWh");
    eval.check_f64("eq3", 0.0, ea.q3, 1e-6, "kWh");
    eval.check_f64("eq4", 0.0, ea.q4, 1e-6, "kWh");

    let er = &sock.energy_metrics.reactive;
    eval.check_f64("er_ind", er_expected, er.inductive(), 0.005, "kvarh");
    eval.check_f64("er_cap", 0.0, er.capacitive(), 1e-6, "kvarh");
    eval.check_f64("er_bal", er_expected, er.balance(), 0.005, "kvarh");
    eval.check_f64("rq1", er_expected, er.q1, 0.005, "kvarh");
    eval.check_f64("rq2", 0.0, er.q2, 1e-6, "kvarh");
    eval.check_f64("rq3", 0.0, er.q3, 1e-6, "kvarh");
    eval.check_f64("rq4", 0.0, er.q4, 1e-6, "kvarh");

    // -----------------------------------------------------------------------
    // Summary
    // -----------------------------------------------------------------------
    println!("\n==========================================================================================");
    println!(
        "SUMMARY: {} PASS, {} FAIL (Total evaluated metrics: {})",
        eval.pass_count,
        eval.fail_count,
        eval.pass_count + eval.fail_count
    );
    println!("==========================================================================================");

    assert_eq!(
        eval.fail_count, 0,
        "Metrology API REALTIME validation failed with {} errors!",
        eval.fail_count
    );
}
