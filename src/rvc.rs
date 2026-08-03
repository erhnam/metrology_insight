//! Rapid Voltage Change (RVC) event detection (IEC 61000-4-30).
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

pub const RVC_MIN_VALID_VOLTAGE_V: f32 = 10.0;
const WINDOW_SIZE: usize = 120; // 120 half-cycles = 100/120-cycle window

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct RvcRecord {
    pub phase_index: u8,
    pub start_timestamp_ns: u64,
    pub end_timestamp_ns: u64,
    pub duration_ms: f32,
    pub delta_u_max_pct: f32,
    pub delta_u_ss_pct: f32,
    pub steady_state_u: f32,
    pub post_event_u: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct RvcConfig {
    pub threshold_pct: f32,
    pub hysteresis_pct: f32,
    pub min_valid_voltage_v: f32,
    pub min_duration_buffers: u8,
}

impl Default for RvcConfig {
    /// Returns an `RvcConfig` with default thresholds (3.0 % threshold, 0.5 % hysteresis).
    fn default() -> Self {
        Self {
            threshold_pct: 3.0,
            hysteresis_pct: 0.5,
            min_valid_voltage_v: RVC_MIN_VALID_VOLTAGE_V,
            min_duration_buffers: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum RvcState {
    Init,
    Ready,
    Active,
    Hysteresis,
}

impl Default for RvcState {
    /// Returns the initial `RvcState` (`Init`).
    fn default() -> Self { RvcState::Init }
}

#[derive(Debug, Clone, Copy)]
pub struct RvcDetector {
    // Circular buffer for Urms(½) window (§5.11)
    urms_buffer: [f32; WINDOW_SIZE],
    buffer_sum: f32,
    buffer_count: u8,
    buffer_idx: u8,

    // State machine
    state: RvcState,
    hysteresis_remaining: u16,

    // Stable signal
    pub voltage_stable: bool,

    // Active event tracking
    pub active_rvc: RvcRecord,
    pub last_completed_rvc: RvcRecord,
    pre_event_mean: f32,
    max_delta_abs_pct: f32,
    stable_since_halfcycles: u16,

    // Counters
    pub rvc_count: u32,
}

impl Default for RvcDetector {
    /// Returns an `RvcDetector` with an empty window buffer, initial state and zeroed counters.
    fn default() -> Self {
        Self {
            urms_buffer: [0.0; WINDOW_SIZE],
            buffer_sum: 0.0,
            buffer_count: 0,
            buffer_idx: 0,
            state: RvcState::default(),
            hysteresis_remaining: 0,
            voltage_stable: false,
            active_rvc: RvcRecord::default(),
            last_completed_rvc: RvcRecord::default(),
            pre_event_mean: 0.0,
            max_delta_abs_pct: 0.0,
            stable_since_halfcycles: 0,
            rvc_count: 0,
        }
    }
}

impl RvcDetector {
    /// Reports whether an RVC event is currently active.
    ///
    /// # Returns
    ///
    /// `true` when the detector is in the `Active` state.
    pub fn is_active(&self) -> bool {
        self.state == RvcState::Active
    }

    /// Processes one half-cycle RMS sample, updating the rolling window and the event state machine.
    ///
    /// # Arguments
    ///
    /// * `phase_index` - Index of the phase this sample belongs to.
    /// * `urms_half` - Half-cycle RMS voltage of the sample.
    /// * `now_ns` - Current timestamp in nanoseconds.
    /// * `config` - Threshold and hysteresis configuration.
    ///
    /// # Returns
    ///
    /// The completed `RvcRecord` when an event ends, or `None` otherwise.
    pub fn process_half_cycle(
        &mut self,
        phase_index: u8,
        urms_half: f32,
        now_ns: u64,
        config: &RvcConfig,
    ) -> Option<RvcRecord> {
        if urms_half <= config.min_valid_voltage_v {
            return None;
        }

        let half_cycle_ns = 10_000_000; // 10 ms at 50 Hz

        // --- Update circular buffer ---
        if self.buffer_count < WINDOW_SIZE as u8 {
            self.urms_buffer[self.buffer_idx as usize] = urms_half;
            self.buffer_sum += urms_half;
            self.buffer_count += 1;
            self.buffer_idx = (self.buffer_idx + 1) % (WINDOW_SIZE as u8);
        } else {
            let old = self.urms_buffer[self.buffer_idx as usize];
            self.urms_buffer[self.buffer_idx as usize] = urms_half;
            self.buffer_sum = self.buffer_sum - old + urms_half;
            self.buffer_idx = (self.buffer_idx + 1) % (WINDOW_SIZE as u8);
        }

        let mean = if self.buffer_count > 0 {
            self.buffer_sum / self.buffer_count as f32
        } else {
            return None;
        };

        // --- Voltage-stable check ---
        let threshold = config.threshold_pct;
        let stable_n = if self.buffer_count == WINDOW_SIZE as u8 {
            WINDOW_SIZE
        } else {
            self.buffer_count as usize
        };
        let mut all_stable = true;
        for i in 0..stable_n {
            let dev = ((self.urms_buffer[i] - mean) / mean).abs() * 100.0;
            if dev >= threshold {
                all_stable = false;
                break;
            }
        }
        self.voltage_stable = all_stable;

        // --- State machine ---
        match self.state {
            RvcState::Init => {
                if self.buffer_count >= WINDOW_SIZE as u8 {
                    self.state = RvcState::Ready;
                }
            }

            RvcState::Ready => {
                if !self.voltage_stable {
                    let dev_pct = ((urms_half - mean) / mean).abs() * 100.0;
                    if dev_pct >= config.threshold_pct {
                        // Event start
                        self.state = RvcState::Active;
                        self.active_rvc = RvcRecord {
                            phase_index,
                            start_timestamp_ns: now_ns,
                            end_timestamp_ns: 0,
                            duration_ms: 0.0,
                            delta_u_max_pct: dev_pct,
                            delta_u_ss_pct: 0.0,
                            steady_state_u: mean,
                            post_event_u: 0.0,
                        };
                        self.pre_event_mean = mean;
                        self.max_delta_abs_pct = dev_pct;
                        self.stable_since_halfcycles = 0;
                    }
                }
            }

            RvcState::Active => {
                // Track max deviation
                let dev_pct = (((urms_half - self.pre_event_mean) / self.pre_event_mean).abs()) * 100.0;
                if dev_pct > self.max_delta_abs_pct {
                    self.max_delta_abs_pct = dev_pct;
                    self.active_rvc.delta_u_max_pct = dev_pct;
                }

                if self.voltage_stable {
                    self.stable_since_halfcycles += 1;
                    // Need window_size stable half-cycles before declaring end
                    if self.stable_since_halfcycles >= WINDOW_SIZE as u16 {
                        // Event ended — end timestamp = now - window_size/2 half-cycles
                        let end_ns = now_ns.saturating_sub((WINDOW_SIZE as u64 / 2) * half_cycle_ns);
                        self.active_rvc.end_timestamp_ns = end_ns;
                        if end_ns > self.active_rvc.start_timestamp_ns {
                            self.active_rvc.duration_ms = (end_ns - self.active_rvc.start_timestamp_ns) as f32 / 1_000_000.0;
                        }
                        self.active_rvc.post_event_u = mean;
                        self.active_rvc.delta_u_ss_pct =
                            ((mean - self.pre_event_mean) / self.pre_event_mean).abs() * 100.0;
                        self.last_completed_rvc = self.active_rvc;

                        self.rvc_count += 1;
                        self.state = RvcState::Hysteresis;
                        self.hysteresis_remaining = WINDOW_SIZE as u16;
                        self.active_rvc = RvcRecord::default();
                        return Some(self.last_completed_rvc);
                    }
                } else {
                    self.stable_since_halfcycles = 0;
                }
            }

            RvcState::Hysteresis => {
                self.hysteresis_remaining -= 1;
                if self.hysteresis_remaining == 0 {
                    self.state = RvcState::Ready;
                }
            }
        }

        None
    }

    /// Discards any in-progress event and moves the detector into the hysteresis state.
    pub fn discard_active(&mut self) {
        if self.state == RvcState::Active {
            self.state = RvcState::Hysteresis;
            self.hysteresis_remaining = WINDOW_SIZE as u16;
            self.active_rvc = RvcRecord::default();
        }
    }

    /// Returns how full the rolling window buffer is.
    ///
    /// # Returns
    ///
    /// The buffer fill percentage from 0 to 100.
    pub fn buffer_fill_pct(&self) -> f32 {
        (self.buffer_count as f32 / WINDOW_SIZE as f32) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies an RVC event is detected, tracked and completed with the correct deviation.
    #[test]
    fn test_rvc_detection() {
        let mut detector = RvcDetector::default();
        let config = RvcConfig::default();

        // Fill buffer with 230V to reach Ready state
        for i in 0..WINDOW_SIZE {
            detector.process_half_cycle(0, 230.0, (i as u64) * 10_000_000, &config);
        }
        assert_eq!(detector.state, RvcState::Ready);
        assert!(detector.voltage_stable);

        // Step to 240V — should trigger event
        let ts = WINDOW_SIZE as u64 * 10_000_000;
        let r = detector.process_half_cycle(0, 240.0, ts, &config);
        assert!(r.is_none());
        assert_eq!(detector.state, RvcState::Active);

        // Return to 230V after a few half-cycles
        for i in 1..=5 {
            let ts = ((WINDOW_SIZE + i) as u64) * 10_000_000;
            detector.process_half_cycle(0, 230.0, ts, &config);
        }
        assert_eq!(detector.state, RvcState::Active);

        // Flush 240V outlier out of buffer (114 writes to reach pos 0),
        // then 120 stable half-cycles for event completion = 234 total
        let complete_at = 2 * WINDOW_SIZE - 1; // = 239
        for i in 6..=complete_at {
            let ts = ((WINDOW_SIZE + i) as u64) * 10_000_000;
            let r = detector.process_half_cycle(0, 230.0, ts, &config);
            if i == complete_at {
                assert!(r.is_some(), "event should complete at i={}", i);
            } else {
                assert!(r.is_none(), "unexpected complete at i={}", i);
            }
        }

        let rec = detector.last_completed_rvc;
        assert!(rec.delta_u_max_pct > 3.0);
        assert_eq!(detector.rvc_count, 1);
    }

    /// Verifies that an in-progress event can be discarded without counting it.
    #[test]
    fn test_rvc_discard() {
        let mut detector = RvcDetector::default();
        let config = RvcConfig::default();

        // Fill buffer
        for i in 0..WINDOW_SIZE {
            detector.process_half_cycle(0, 230.0, (i as u64) * 10_000_000, &config);
        }

        // Trigger event
        detector.process_half_cycle(0, 240.0, (WINDOW_SIZE as u64) * 10_000_000, &config);
        assert_eq!(detector.state, RvcState::Active);

        // Discard
        detector.discard_active();
        assert_eq!(detector.state, RvcState::Hysteresis);
        assert_eq!(detector.rvc_count, 0);
    }
}
