//! Debug print helpers for metrology metrics.
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

use alloc::format;
use alloc::vec::Vec;

use crate::MetrologyInsightSocket;

const PHASE_LABELS: [&str; 4] = ["A", "B", "C", "N"];

/// Logs the voltage signal values (peak, RMS, half-cycle RMS and flicker) for each active
/// phase, plus the PLL frequency.
///
/// # Arguments
///
/// * `data` - Socket with the measured phase data.
/// * `active` - Number of active phases to print.
pub fn print_voltage_signal(data: &MetrologyInsightSocket, active: usize) {
    log::info!("Voltage:");
    for (i, phase) in data.phases.iter().enumerate().take(active.min(3)) {
        log::info!(
            "  Phase {}: Peak: {:.3} V, RMS: {:.3} V, Urms(1/2): {:.3} V",
            PHASE_LABELS[i],
            phase.voltage.peak,
            phase.voltage.rms,
            phase.voltage.urms_half_cycle.urms,
        );
        log::info!(
            "            Flicker (P_inst): {:.3}",
            phase.flicker_meter.p_inst,
        );
    }
    if active > 0 {
        log::info!(
            "  Frequency: {:.3} Hz (PLL Lock)\n",
            data.phases[0].voltage.pll_state.freq_est,
        );
    }
}

/// Logs the current RMS and half-cycle RMS for each active phase.
///
/// # Arguments
///
/// * `data` - Socket with the measured phase data.
/// * `active` - Number of active phases to print.
pub fn print_current_signal(data: &MetrologyInsightSocket, active: usize) {
    log::info!("Current:");
    for (i, phase) in data.phases.iter().enumerate().take(active) {
        log::info!(
            "  Phase {} RMS: {:.3} A, Irms(1/2): {:.3} A",
            PHASE_LABELS[i],
            phase.current.rms,
            phase.current.urms_half_cycle.urms,
        );
    }
}

/// Logs the voltage and current THD plus harmonic components for each active phase.
///
/// # Arguments
///
/// * `data` - Socket with the measured phase data.
/// * `active` - Number of active phases to print.
pub fn print_harmonics(data: &MetrologyInsightSocket, active: usize) {
    for (i, phase) in data.phases.iter().enumerate().take(active.min(3)) {
        log::info!("Voltage Harmonics (Phase {}):", PHASE_LABELS[i]);
        log::info!("  THD: {:.3} %", phase.voltage.thd);
        log::info!(
            "  Harmonics: [{}]",
            phase
                .voltage
                .harmonics
                .iter()
                .map(|h| format!("{:.3}", h))
                .collect::<Vec<_>>()
                .join(", ")
        );
        log::info!("Current Harmonics (Phase {}):", PHASE_LABELS[i]);
        log::info!("  THD: {:.3} %", phase.current.thd);
        log::info!(
            "  Harmonics: [{}]",
            phase
                .current
                .harmonics
                .iter()
                .map(|h| format!("{:.3}", h))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}

/// Logs the total active, reactive and apparent power and the power factor.
///
/// # Arguments
///
/// * `data` - Socket with the measured power data.
pub fn print_power(data: &MetrologyInsightSocket) {
    log::info!("Power (Total):");
    log::info!("  Active: {:.3} W", data.power_metrics_total.real_power);
    log::info!(
        "  Reactive: {:.3} VAR",
        data.power_metrics_total.reactive_power
    );
    log::info!(
        "  Apparent: {:.3} VA",
        data.power_metrics_total.apparent_power
    );
    log::info!("  Factor: {:.3}\n", data.power_metrics_total.power_factor);
}

/// Logs the current-to-voltage, voltage and current angles and the phase direction for each
/// active phase.
///
/// # Arguments
///
/// * `data` - Socket with the measured phase data.
/// * `active` - Number of active phases to print.
pub fn print_phase_angle(data: &MetrologyInsightSocket, active: usize) {
    for (i, phase) in data.phases.iter().enumerate().take(active.min(3)) {
        log::info!("Phase Angle (Phase {}):", PHASE_LABELS[i]);
        log::info!(
            "  Current to Voltage Angle: {:.2}º",
            phase.phase_angles.c2v_angle
        );
        log::info!("  Voltage Angle: {:.2}º", phase.phase_angles.v_angle);
        log::info!("  Current Angle: {:.2}º", phase.phase_angles.c_angle);
        log::info!(
            "  Phase direction: {}",
            phase.phase_angles.direction_description()
        );
    }
}

/// Logs the inter-phase voltage angles A-B, B-C and C-A for three-phase systems.
///
/// # Arguments
///
/// * `data` - Socket with the measured phase data.
/// * `active` - Number of active phases to print.
pub fn print_interphase_angle(data: &MetrologyInsightSocket, active: usize) {
    if active >= 3 {
        log::info!("Inter-phase Angles:");
        log::info!(
            "  A-B: {:.1}º",
            data.phases[0].phase_angles.v_angle - data.phases[1].phase_angles.v_angle
        );
        log::info!(
            "  B-C: {:.1}º",
            data.phases[1].phase_angles.v_angle - data.phases[2].phase_angles.v_angle
        );
        log::info!(
            "  C-A: {:.1}º",
            data.phases[2].phase_angles.v_angle - data.phases[0].phase_angles.v_angle
        );
    }
}

/// Logs the imported/exported active energy, balance and quadrant energies.
///
/// # Arguments
///
/// * `data` - Socket with the measured energy data.
pub fn print_active_energy(data: &MetrologyInsightSocket) {
    log::info!("Active Energy:");
    log::info!(
        "  Imported Energy: {:.3} kWh",
        data.energy_metrics.active.imported
    );
    log::info!(
        "  Exported Energy: {:.3} kWh",
        data.energy_metrics.active.exported
    );
    log::info!("  Balance: {:.3} kWh\n", data.energy_metrics.active.balance);
    log::info!(
        "  Active Energy Q1: {:.3} kWh",
        data.energy_metrics.active.q1
    );
    log::info!(
        "  Active Energy Q2: {:.3} kWh",
        data.energy_metrics.active.q2
    );
    log::info!(
        "  Active Energy Q3: {:.3} kWh",
        data.energy_metrics.active.q3
    );
    log::info!(
        "  Active Energy Q4: {:.3} kWh\n",
        data.energy_metrics.active.q4
    );
}

/// Logs the capacitive/inductive reactive energy, balance and quadrant energies.
///
/// # Arguments
///
/// * `data` - Socket with the measured energy data.
pub fn print_reactive_energy(data: &MetrologyInsightSocket) {
    log::info!("Reactive Energy:");
    log::info!(
        "  Capacitive Energy: {:.3} kWh",
        data.energy_metrics.reactive.capacitive
    );
    log::info!(
        "  Inductive Energy: {:.3} kWh",
        &data.energy_metrics.reactive.inductive
    );
    log::info!(
        "  Balance: {:.3} kWh\n",
        data.energy_metrics.reactive.balance
    );
    log::info!(
        "  Reactive Energy Q1: {:.3} kWh",
        data.energy_metrics.reactive.q1
    );
    log::info!(
        "  Reactive Energy Q2: {:.3} kWh",
        data.energy_metrics.reactive.q2
    );
    log::info!(
        "  Reactive Energy Q3: {:.3} kWh",
        data.energy_metrics.reactive.q3
    );
    log::info!(
        "  Reactive Energy Q4: {:.3} kWh\n",
        data.energy_metrics.reactive.q4
    );
}

/// Logs the voltage and current unbalance ratios and the zero/positive/negative sequence
/// currents for three-phase systems.
///
/// # Arguments
///
/// * `data` - Socket with the measured unbalance data.
/// * `active` - Number of active phases to print.
pub fn print_unbalance(data: &MetrologyInsightSocket, active: usize) {
    if active >= 3 {
        log::info!("Unbalance:");
        log::info!(
            "  Voltage u2: {:.2}%  u0: {:.2}%",
            data.unbalance_metrics.u2_neg_ratio_pct,
            data.unbalance_metrics.u0_zero_ratio_pct
        );
        log::info!(
            "  Current u2: {:.2}%  u0: {:.2}%",
            data.unbalance_metrics.u2_i_ratio_pct,
            data.unbalance_metrics.u0_i_ratio_pct
        );
        log::info!(
            "  I0: {:.4} A  I1: {:.4} A  I2: {:.4} A\n",
            data.unbalance_metrics.i0_zero_seq,
            data.unbalance_metrics.i1_pos_seq,
            data.unbalance_metrics.i2_neg_seq
        );
    }
}

/// Logs the accumulated dip, swell, interruption and RVC counts, plus the maximum RVC ΔU.
///
/// # Arguments
///
/// * `data` - Socket with the measured event data.
/// * `active` - Number of active phases to print.
pub fn print_events(data: &MetrologyInsightSocket, active: usize) {
    if active == 0 {
        return;
    }
    let mut dip = 0u32;
    let mut swell = 0u32;
    let mut interrupt = 0u32;
    let mut rvc = 0u32;
    let mut max_delta = 0.0f32;
    for i in 0..active.min(3) {
        dip += data.phases[i].event_detector.dip_count;
        swell += data.phases[i].event_detector.swell_count;
        interrupt += data.phases[i].event_detector.interruption_count;
        rvc += data.phases[i].rvc_detector.rvc_count;
        let last = &data.phases[i].rvc_detector.last_completed_rvc;
        if last.delta_u_max_pct > max_delta {
            max_delta = last.delta_u_max_pct;
        }
    }
    log::info!("Events:");
    log::info!(
        "  Dips: {}  Swells: {}  Interruptions: {}  RVC: {}\n",
        dip,
        swell,
        interrupt,
        rvc
    );
    if max_delta > 0.0 {
        log::info!("  RVC max ΔU: {:.2}%\n", max_delta);
    }
}

/// Logs all metrology sections: signals, harmonics, power, angles, unbalance, events and energies.
///
/// # Arguments
///
/// * `data` - Socket with the measured metrology data.
/// * `active_phases` - Number of active phases to print.
pub fn print_all(data: &MetrologyInsightSocket, active_phases: usize) {
    print_voltage_signal(data, active_phases);
    print_current_signal(data, active_phases);
    print_harmonics(data, active_phases);
    print_power(data);
    print_phase_angle(data, active_phases);
    print_interphase_angle(data, active_phases);
    print_unbalance(data, active_phases);
    print_events(data, active_phases);
    print_active_energy(data);
    print_reactive_energy(data);
}
