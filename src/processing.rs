//! Main DSP pipeline coordinating per-cycle metric updates.
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    print_all, process_signal, update_average, update_phase_angles, update_power_metrics, update_total_energy, FftCache,
    MetrologyInsight,
};

// Default balanced 3-phase angles (degrees) used as fallback when PLL is not yet locked.
// Phase order: L1=0°, L2=+120°, L3=+240°.
const THREE_PHASE_DEFAULT_ANGLES: [f32; 3] = [0.0, 120.0, 240.0];

impl MetrologyInsight {
    /// Processes all signals for the active phases and updates the socket metrics
    /// (harmonics, interharmonics, flicker, events, RVC, power, energy and unbalance).
    ///
    /// # Arguments
    ///
    /// * `active_phases` - Number of active phases (1..=4) to process this frame.
    pub fn process_and_update_metrics(&mut self, active_phases: usize) {
        self.active_phases = active_phases;
        if self.fft_cache.is_none() {
            self.fft_cache = Some(FftCache::new(crate::harmonics::FFT_RESOLUTION));
        }
        let cache = self.fft_cache.as_mut().unwrap();

        for i in 0..active_phases {
            if i < 3 {
                let phase_delay = self.config.calibration.phase_delay_us[i];

                process_signal(
                    &mut self.socket.phases[i].voltage,
                    0.0,
                    0.0,
                    &self.config,
                    cache,
                );

                // Interharmonic accumulation (§5.5): push voltage sync buffer each cycle
                let phase = &mut self.socket.phases[i];
                phase.interharm_acc.push_cycle(cache.sync_buffer.as_ref());
                if phase.interharm_acc.is_ready() {
                    let fund_mag = phase.voltage.rms;
                    if let Some(inter) = phase.interharm_acc.compute(fund_mag) {
                        phase.voltage.interharmonics = inter;
                    }
                }

                for &v in self.socket.phases[i].voltage.real_wave_slice() {
                    self.socket.phases[i].flicker_meter.process_sample(v, self.config.adc_samples_seconds);
                }

                let v_freq_pll = self.socket.phases[i].voltage.pll_state.freq_est;

                let urms_half = self.socket.phases[i].voltage.urms_half_cycle.urms;
                let frame_ns = self.socket.phases[i].voltage.frame_start_ns;
                if urms_half > 0.0 {
                    let prev_event_active = self.socket.phases[i].event_detector.active_event.is_active;
                    self.socket.phases[i].event_detector.process_half_cycle(
                        i as u8,
                        urms_half,
                        frame_ns,
                        &self.config.event_config,
                    );
                    // Discard RVC if a dip/swell/interruption started
                    if !prev_event_active && self.socket.phases[i].event_detector.active_event.is_active {
                        self.socket.phases[i].rvc_detector.discard_active();
                    }
                    self.socket.phases[i].rvc_detector.process_half_cycle(
                        i as u8,
                        urms_half,
                        frame_ns,
                        &self.config.rvc_config,
                    );
                }
                if self.socket.phases[i].event_detector.active_event.is_active || self.socket.phases[i].rvc_detector.is_active() {
                    self.socket.phases[i].voltage.quality_flags |= crate::types::Q_FLAG_EVENT_MARKED;
                }

                process_signal(
                    &mut self.socket.phases[i].current,
                    v_freq_pll,
                    phase_delay,
                    &self.config,
                    cache,
                );
                if self.socket.phases[i].event_detector.active_event.is_active || self.socket.phases[i].rvc_detector.is_active() {
                    self.socket.phases[i].current.quality_flags |= crate::types::Q_FLAG_EVENT_MARKED;
                }
            } else {
                let current_slice = self.socket.phases[i].current.real_wave_slice();
                let sum_sq: f32 = current_slice.iter().map(|&s| s * s).sum();
                let rms = if !current_slice.is_empty() { (sum_sq / current_slice.len() as f32).sqrt() } else { 0.0 };
                update_average(rms, &mut self.socket.phases[i].current.rms, self.config.avg_sec);
                self.socket.phases[i].current.peak = 0.0;
                self.socket.phases[i].current.thd = 0.0;
                self.socket.phases[i].current.harmonics = [0.0; crate::types::NUMBER_HARMONICS];
                self.socket.phases[i].current.interharmonics = [0.0; crate::types::NUMBER_INTERHARMONICS];
                self.socket.phases[i].voltage.rms = 0.0;
                self.socket.phases[i].voltage.peak = 0.0;
                self.socket.phases[i].voltage.thd = 0.0;
                self.socket.phases[i].voltage.harmonics = [0.0; crate::types::NUMBER_HARMONICS];
                self.socket.phases[i].voltage.interharmonics = [0.0; crate::types::NUMBER_INTERHARMONICS];
            }
        }

        update_phase_angles(&mut self.socket, self.config.adc_samples_seconds, active_phases);
        update_power_metrics(&mut self.socket, active_phases);

        let noise_threshold = self.config.standard_values.ist_a * 0.4;
        let any_above_noise = (0..active_phases).any(|i| {
            self.socket.phases[i].current.rms > noise_threshold
        });
        if any_above_noise {
            update_total_energy(&mut self.socket, self.config.adc_samples_seconds as f64, active_phases);
        }
        if active_phases >= 3 {
            let p0_locked = self.socket.phases[0].voltage.pll_state.locked;
            let p1_locked = self.socket.phases[1].voltage.pll_state.locked;
            let p2_locked = self.socket.phases[2].voltage.pll_state.locked;
            let all_locked = p0_locked && p1_locked && p2_locked;

            let v_rms = [
                self.socket.phases[0].voltage.rms,
                self.socket.phases[1].voltage.rms,
                self.socket.phases[2].voltage.rms,
            ];
            let v_angles = if all_locked {
                [
                    self.socket.phases[0].phase_angles.v_angle,
                    self.socket.phases[1].phase_angles.v_angle,
                    self.socket.phases[2].phase_angles.v_angle,
                ]
            } else {
                THREE_PHASE_DEFAULT_ANGLES
            };
            let mut unbalance = crate::unbalance::calculate_voltage_unbalance(&v_rms, &v_angles);

            // Current unbalance (§5.13.6)
            let i_rms = [
                self.socket.phases[0].current.rms,
                self.socket.phases[1].current.rms,
                self.socket.phases[2].current.rms,
            ];
            let i_angles = if all_locked {
                [
                    self.socket.phases[0].phase_angles.c_angle,
                    self.socket.phases[1].phase_angles.c_angle,
                    self.socket.phases[2].phase_angles.c_angle,
                ]
            } else {
                THREE_PHASE_DEFAULT_ANGLES
            };
            let i_unb = crate::unbalance::calculate_current_unbalance(&i_rms, &i_angles);
            unbalance.i0_zero_seq = i_unb.i0_zero_seq;
            unbalance.i1_pos_seq = i_unb.i1_pos_seq;
            unbalance.i2_neg_seq = i_unb.i2_neg_seq;
            unbalance.u2_i_ratio_pct = i_unb.u2_i_ratio_pct;
            unbalance.u0_i_ratio_pct = i_unb.u0_i_ratio_pct;

            self.socket.unbalance_metrics = unbalance;
        } else {
            self.socket.unbalance_metrics = crate::unbalance::UnbalanceMetrics::default();
        }
    }

    /// Prints the metrology report for all active phases via `print_all`.
    pub fn print_metrology_report(&mut self) {
        print_all(&self.socket, self.active_phases);
    }
}
