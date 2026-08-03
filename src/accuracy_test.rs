//! Per-phase accuracy test harness and test point definitions (IEC 62053-21).
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    CalibrationFactors, MetrologyInsight, MetrologyInsightConfig, PhaseConfig, PllConfig,
    SignalConfig, MAX_SIGNAL_SAMPLES, FREQ_NOMINAL_50, FREQ_NOMINAL_60,
};

/// Per-phase test configuration for polyphase accuracy tests.
#[derive(Debug, Clone, Copy)]
pub struct PhaseTestPoint {
    pub v_rms: f32,
    pub i_rms: f32,
    pub pf: f32,
}

/// Result of an accuracy test run.
pub struct AccuracyTestResult {
    pub v_rms: f32,
    pub i_rms: f32,
    pub pf: f32,
    pub freq: f32,
    pub cycles: u32,
    pub energy_ref_wh: f64,
    pub energy_meas_wh: f64,
    pub error_pct: f64,
}

/// Result of a polyphase accuracy test run.
pub struct PolyphaseTestResult {
    pub phases: [PhaseTestPoint; 3],
    pub freq: f32,
    pub cycles: u32,
    pub energy_ref_wh: f64,
    pub energy_meas_wh: f64,
    pub error_pct: f64,
}

/// Builds a test configuration derived from the sampling rate and signal frequency.
///
/// # Arguments
///
/// * `fs` - Sampling rate.
/// * `freq` - Signal frequency, used to select the nominal frequency and per-cycle settings.
///
/// # Returns
///
/// A fully populated `MetrologyInsightConfig` for accuracy testing.
fn make_test_config(fs: f32, freq: f32) -> MetrologyInsightConfig {
    let nominal = if (freq - FREQ_NOMINAL_50).abs() < 0.1 {
        FREQ_NOMINAL_50
    } else {
        FREQ_NOMINAL_60
    };
    MetrologyInsightConfig {
        avg_sec: (fs / freq).recip(),
        adc_samples_seconds: fs,
        adc_samples_per_cycle: (fs / freq) as f64,
        nominal_freq: nominal,
        min_amplitude_voltage: 1.0,
        min_amplitude_current: 0.0001,
        calibration: CalibrationFactors {
            v_gain: 1.0,
            i_gain: [1.0, 1.0, 1.0],
            phase_offset: [0.0, 0.0, 0.0],
            phase_delay_us: [0.0, 0.0, 0.0],
            temp_coeff: 0.0,
            v_lsb_to_phys: 1.0,
            i_lsb_to_phys: 1.0,
        },
        pll: PllConfig {
            kp: 0.002,
            ki: 0.00005,
            freq_min: nominal * 0.95,
            freq_max: nominal * 1.05,
            lock_threshold: 0.5,
            norm_threshold: 1.0,
            integrator_clamp: 0.1,
            lock_ema_alpha: 0.1,
        },
        phase: PhaseConfig { direction_deadband_deg: 10.0 },
        signal: SignalConfig {
            half_cycle_min_factor: 0.4,
            rms_consistency_min_guard: 1e-6,
            pll_error_accum_threshold: 0.5,
            sync_consistency_threshold: 0.001,
        },
        ..MetrologyInsightConfig::default()
    }
}

/// Copies one cycle of V and I samples into the insight's phase buffer.
///
/// # Arguments
///
/// * `insight` - The metrology instance to fill.
/// * `v_samples` - Voltage samples for the cycle.
/// * `i_samples` - Current samples for the cycle.
/// * `phase_idx` - Target phase index.
fn push_cycle(
    insight: &mut MetrologyInsight,
    v_samples: &[f32],
    i_samples: &[f32],
    phase_idx: usize,
) {
    let n = v_samples.len().min(i_samples.len()).min(MAX_SIGNAL_SAMPLES);
    let phase = &mut insight.socket.phases[phase_idx];
    phase.voltage.real_wave[..n].copy_from_slice(&v_samples[..n]);
    phase.voltage.real_wave_len = n;
    phase.current.real_wave[..n].copy_from_slice(&i_samples[..n]);
    phase.current.real_wave_len = n;
}

/// Clears the buffered V/I samples for a given phase.
///
/// # Arguments
///
/// * `insight` - The metrology instance to clear.
/// * `phase_idx` - Phase index to clear.
fn clear_cycle(insight: &mut MetrologyInsight, phase_idx: usize) {
    insight.socket.phases[phase_idx].voltage.clear_samples();
    insight.socket.phases[phase_idx].current.clear_samples();
}

/// Computes the net active energy in Wh from the insight's accumulated metrics.
///
/// # Arguments
///
/// * `insight` - Metrology instance with updated energy metrics.
///
/// # Returns
///
/// The imported-minus-exported active energy in watt-hours.
fn energy_wh(insight: &MetrologyInsight) -> f64 {
    // imported() / exported() return kWh; convert to Wh
    (insight.socket.energy_metrics.active.imported()
        - insight.socket.energy_metrics.active.exported()) * 1000.0
}

/// Generates one cycle of pure-sine voltage and current waveforms at the given power factor.
///
/// # Arguments
///
/// * `v_rms` - RMS voltage of the cycle.
/// * `i_rms` - RMS current of the cycle.
/// * `pf` - Power factor (cosine of the phase angle between V and I).
/// * `freq` - Fundamental frequency of the cycle.
/// * `fs` - Sampling rate.
///
/// # Returns
///
/// A tuple of the voltage and current sample vectors, each one full cycle long.
pub fn generate_cycle(
    v_rms: f32,
    i_rms: f32,
    pf: f32,
    freq: f32,
    fs: f32,
) -> (alloc::vec::Vec<f32>, alloc::vec::Vec<f32>) {
    let n = (fs / freq).round() as usize;
    let v_peak = v_rms * core::f32::consts::SQRT_2;
    let i_peak = i_rms * core::f32::consts::SQRT_2;
    let phi = pf.acos();
    use core::f32::consts::PI;
    let v: alloc::vec::Vec<f32> = (0..n)
        .map(|i| v_peak * (2.0 * PI * freq / fs * i as f32).sin())
        .collect();
    let i: alloc::vec::Vec<f32> = (0..n)
        .map(|i| i_peak * (2.0 * PI * freq / fs * i as f32 - phi).sin())
        .collect();
    (v, i)
}

/// Harmonic content per IEC 62053-21 §9.4.4 typical test:
/// 3rd = 20 %, 5th = 10 %, 7th = 5 % of fundamental.
const HARMONIC_AMPS: &[(u32, f32)] = &[(3, 0.20), (5, 0.10), (7, 0.05)];

/// Generates one cycle of V (pure sine) and I (fundamental + harmonics) at PF=1.
///
/// # Arguments
///
/// * `v_rms` - RMS voltage of the cycle.
/// * `i_rms` - RMS current of the cycle.
/// * `freq` - Fundamental frequency.
/// * `fs` - Sampling rate.
///
/// # Returns
///
/// A tuple of the voltage and current sample vectors, each one full cycle long.
pub fn generate_cycle_with_harmonics(
    v_rms: f32,
    i_rms: f32,
    freq: f32,
    fs: f32,
) -> (alloc::vec::Vec<f32>, alloc::vec::Vec<f32>) {
    let n = (fs / freq).round() as usize;
    let v_peak = v_rms * core::f32::consts::SQRT_2;
    let i_peak = i_rms * core::f32::consts::SQRT_2;
    use core::f32::consts::PI;
    let v: alloc::vec::Vec<f32> = (0..n)
        .map(|i| v_peak * (2.0 * PI * freq / fs * i as f32).sin())
        .collect();
    let i: alloc::vec::Vec<f32> = (0..n)
        .map(|i| {
            let t = 2.0 * PI * freq / fs * i as f32;
            let fund = i_peak * t.sin();
            let harm: f32 = HARMONIC_AMPS
                .iter()
                .map(|&(h, a)| i_peak * a * (h as f32 * t).sin())
                .sum();
            fund + harm
        })
        .collect();
    (v, i)
}

/// Generates one cycle with half-wave rectified current (DC component test).
///
/// # Arguments
///
/// * `v_rms` - RMS voltage of the cycle.
/// * `i_rms` - RMS current of the cycle.
/// * `pf` - Power factor (cosine of the phase angle between V and I).
/// * `freq` - Fundamental frequency.
/// * `fs` - Sampling rate.
///
/// # Returns
///
/// A tuple of the voltage and current sample vectors, with current zeroed on negative half-cycles.
pub fn generate_half_wave_cycle(
    v_rms: f32,
    i_rms: f32,
    pf: f32,
    freq: f32,
    fs: f32,
) -> (alloc::vec::Vec<f32>, alloc::vec::Vec<f32>) {
    let n = (fs / freq).round() as usize;
    let v_peak = v_rms * core::f32::consts::SQRT_2;
    let i_peak = i_rms * core::f32::consts::SQRT_2;
    let phi = pf.acos();
    use core::f32::consts::PI;
    let v: alloc::vec::Vec<f32> = (0..n)
        .map(|i| v_peak * (2.0 * PI * freq / fs * i as f32).sin())
        .collect();
    let i: alloc::vec::Vec<f32> = (0..n)
        .map(|i| {
            let val = i_peak * (2.0 * PI * freq / fs * i as f32 - phi).sin();
            val.max(0.0)
        })
        .collect();
    (v, i)
}

/// Runs a polyphase accuracy test over three phases and returns the measured vs. reference error.
///
/// # Arguments
///
/// * `phases` - Per-phase test points (V RMS, I RMS, PF).
/// * `freq` - Nominal signal frequency.
/// * `cycles` - Number of cycles to integrate.
///
/// # Returns
///
/// The polyphase test result with reference/measured energy and percent error.
pub fn run_polyphase_accuracy_test(
    phases: [PhaseTestPoint; 3],
    freq: f32,
    cycles: u32,
) -> PolyphaseTestResult {
    let fs = 8000.0;
    let mut cfg = make_test_config(fs, freq);
    cfg.standard_values.un_v = phases[0].v_rms;
    cfg.standard_values.in_a = phases[0].i_rms;
    cfg.standard_values.fn_hz = freq;

    let mut insight = MetrologyInsight::new(cfg);

    let dt_s = 1.0 / freq;
    let time_s = dt_s as f64 * cycles as f64;

    for _ in 0..cycles {
        for p in 0..3 {
            let (v, i) = generate_cycle(phases[p].v_rms, phases[p].i_rms, phases[p].pf, freq, fs);
            push_cycle(&mut insight, &v, &i, p);
        }
        insight.process_and_update_metrics(3);
        for p in 0..3 {
            clear_cycle(&mut insight, p);
        }
    }

    let energy_meas_wh = energy_wh(&insight);

    let energy_ref_wh: f64 = phases
        .iter()
        .map(|ph| (ph.v_rms as f64) * (ph.i_rms as f64) * (ph.pf as f64) * time_s / 3600.0)
        .sum();

    let error_pct = if energy_ref_wh.abs() > 1e-12 {
        (energy_meas_wh - energy_ref_wh) / energy_ref_wh * 100.0
    } else {
        0.0
    };

    PolyphaseTestResult {
        phases,
        freq,
        cycles,
        energy_ref_wh,
        energy_meas_wh,
        error_pct,
    }
}

/// Runs a single-phase accuracy test and returns the measured vs. reference error.
///
/// # Arguments
///
/// * `v_rms` - RMS voltage.
/// * `i_rms` - RMS current.
/// * `pf` - Power factor.
/// * `freq` - Nominal signal frequency.
/// * `cycles` - Number of cycles to integrate.
///
/// # Returns
///
/// The accuracy test result with reference/measured energy and percent error.
pub fn run_accuracy_test(
    v_rms: f32,
    i_rms: f32,
    pf: f32,
    freq: f32,
    cycles: u32,
) -> AccuracyTestResult {
    let fs = 8000.0;
    let mut insight = make_test_config(fs, freq);
    insight.standard_values.un_v = v_rms;
    insight.standard_values.in_a = i_rms;
    insight.standard_values.fn_hz = freq;

    let mut insight = MetrologyInsight::new(insight);

    let dt_s = 1.0 / freq;
    let time_s = dt_s as f64 * cycles as f64;

    for _ in 0..cycles {
        let (v, i) = generate_cycle(v_rms, i_rms, pf, freq, fs);
        for p in 0..insight.active_phases {
            push_cycle(&mut insight, &v, &i, p);
        }
        insight.process_and_update_metrics(insight.active_phases);
        for p in 0..insight.active_phases {
            clear_cycle(&mut insight, p);
        }
    }

    let energy_meas_wh = energy_wh(&insight);
    let energy_ref_wh = (v_rms as f64) * (i_rms as f64) * (pf as f64) * time_s / 3600.0;

    let error_pct = if energy_ref_wh.abs() > 1e-12 {
        (energy_meas_wh - energy_ref_wh) / energy_ref_wh * 100.0
    } else {
        0.0
    };

    AccuracyTestResult {
        v_rms,
        i_rms,
        pf,
        freq,
        cycles,
        energy_ref_wh,
        energy_meas_wh,
        error_pct,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks that the polyphase accuracy error is below 1 % for a balanced load.
    #[test]
    fn test_polyphase_balanced() {
        let ph = PhaseTestPoint { v_rms: 230.0, i_rms: 5.0, pf: 1.0 };
        let r = run_polyphase_accuracy_test([ph, ph, ph], 50.0, 100);
        assert!(
            r.error_pct.abs() < 1.0,
            "Balanced 3-phase error: {:.4}%",
            r.error_pct
        );
    }

    /// Checks the polyphase accuracy error when only phase 0 is loaded.
    #[test]
    fn test_polyphase_unbalanced_phase0_only() {
        let loaded = PhaseTestPoint { v_rms: 230.0, i_rms: 5.0, pf: 1.0 };
        let unloaded = PhaseTestPoint { v_rms: 230.0, i_rms: 0.0, pf: 1.0 };
        let r = run_polyphase_accuracy_test([loaded, unloaded, unloaded], 50.0, 200);
        assert!(
            r.error_pct.abs() < 1.0,
            "Unbalanced (only phase 0 loaded) error: {:.4}%",
            r.error_pct
        );
    }

    /// Checks the polyphase accuracy error for three phases with different loads and power factors.
    #[test]
    fn test_polyphase_unbalanced_uneven() {
        let ph0 = PhaseTestPoint { v_rms: 230.0, i_rms: 5.0, pf: 1.0 };
        let ph1 = PhaseTestPoint { v_rms: 230.0, i_rms: 2.5, pf: 0.8 };
        let ph2 = PhaseTestPoint { v_rms: 230.0, i_rms: 1.0, pf: 0.5 };
        let r = run_polyphase_accuracy_test([ph0, ph1, ph2], 50.0, 200);
        assert!(
            r.error_pct.abs() < 1.0,
            "Uneven 3-phase error: {:.4}%",
            r.error_pct
        );
    }

    /// Checks the single-phase accuracy error at 50 Hz with PF=1.
    #[test]
    fn test_balanced_reference_50hz() {
        let r = run_accuracy_test(230.0, 5.0, 1.0, 50.0, 100);
        assert!(
            r.error_pct.abs() < 1.0,
            "Error at reference (PF=1.0): {:.4}%",
            r.error_pct
        );
    }

    /// Checks the single-phase accuracy error at 60 Hz with PF=1.
    #[test]
    fn test_balanced_reference_60hz() {
        let r = run_accuracy_test(230.0, 5.0, 1.0, 60.0, 120);
        assert!(
            r.error_pct.abs() < 1.0,
            "Error at reference 60 Hz: {:.4}%",
            r.error_pct
        );
    }

    /// Checks the accuracy error at inductive PF=0.5.
    #[test]
    fn test_inductive_pf_05() {
        let r = run_accuracy_test(230.0, 5.0, 0.5, 50.0, 100);
        assert!(
            r.error_pct.abs() < 1.5,
            "Error at PF=0.5 inductive: {:.4}%",
            r.error_pct
        );
    }

    /// Checks the accuracy error at capacitive PF=0.8.
    #[test]
    fn test_capacitive_pf_08() {
        let r = run_accuracy_test(230.0, 5.0, 0.8, 50.0, 100);
        assert!(
            r.error_pct.abs() < 1.5,
            "Error at PF=0.8 capacitive: {:.4}%",
            r.error_pct
        );
    }

    /// Checks the accuracy error at 5 % of nominal current (0.25 A).
    #[test]
    fn test_low_current_5pct() {
        let r = run_accuracy_test(230.0, 0.25, 1.0, 50.0, 200);
        assert!(
            r.error_pct.abs() < 2.0,
            "Error at 5% In (0.25 A): {:.4}%",
            r.error_pct
        );
    }

    /// Checks that repeated runs of the same test produce consistent errors.
    #[test]
    fn test_error_repeatability() {
        let results: alloc::vec::Vec<f64> = (0..5)
            .map(|_| run_accuracy_test(230.0, 5.0, 1.0, 50.0, 50).error_pct)
            .collect();
        let mean = results.iter().copied().sum::<f64>() / results.len() as f64;
        let variance = results
            .iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>()
            / results.len() as f64;
        let std_dev = variance.sqrt();
        assert!(
            std_dev < 0.05,
            "Repeatability std_dev too high: {:.6}%",
            std_dev
        );
    }
}
