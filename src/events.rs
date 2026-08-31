//! Power quality event type definitions (dip, swell, interruption).
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PqEventType {
    #[default]
    None,
    Dip,          // Urms(1/2) < 90% Udin
    Swell,        // Urms(1/2) > 110% Udin
    Interruption, // Urms(1/2) < 10% Udin
}

impl PqEventType {
    /// Returns the string name of this event type.
    ///
    /// # Returns
    ///
    /// A static string such as "Dip", "Swell", "Interruption" or "None".
    pub fn as_str(&self) -> &'static str {
        match self {
            PqEventType::None => "None",
            PqEventType::Dip => "Dip",
            PqEventType::Swell => "Swell",
            PqEventType::Interruption => "Interruption",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct PqEventRecord {
    pub event_type: PqEventType,
    pub phase_index: u8,
    pub start_timestamp_ns: u64,
    pub duration_ms: f32,
    pub extremum_v: f32, // Min voltage for Dip/Interruption, max voltage for Swell
    pub reference_v: f32, // Nominal voltage (e.g. 230.0 V)
    pub is_active: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct PqEventConfig {
    pub nominal_voltage: f32,         // Default 230.0 V
    pub dip_threshold_pct: f32,       // Default 90.0%
    pub swell_threshold_pct: f32,     // Default 110.0%
    pub interrupt_threshold_pct: f32, // Default 10.0%
    pub hysteresis_pct: f32,          // Default 1.0%
}

impl Default for PqEventConfig {
    /// Returns a `PqEventConfig` with IEC-typical defaults (230.0 V nominal, 90 %/110 %/10 %
    /// thresholds and 1.0 % hysteresis).
    fn default() -> Self {
        Self {
            nominal_voltage: 230.0,
            dip_threshold_pct: 90.0,
            swell_threshold_pct: 110.0,
            interrupt_threshold_pct: 10.0,
            hysteresis_pct: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PowerQualityEventDetector {
    pub active_event: PqEventRecord,
    pub last_completed_event: PqEventRecord,
    pub event_count: u32,
    pub dip_count: u32,
    pub swell_count: u32,
    pub interruption_count: u32,
    pub pending_type: PqEventType,
    pub pending_count: u8,
    pub pending_start_ns: u64,
}

impl PowerQualityEventDetector {
    /// Processes one half-cycle RMS sample and updates any active power quality event.
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
    /// The just-completed `PqEventRecord`, or `None` when no event finished on this call.
    pub fn process_half_cycle(
        &mut self,
        phase_index: u8,
        urms_half: f32,
        now_ns: u64,
        config: &PqEventConfig,
    ) -> Option<PqEventRecord> {
        let u_din = config.nominal_voltage;
        if u_din <= 0.0 || urms_half <= 0.0 {
            return None;
        }

        let dip_thresh = u_din * (config.dip_threshold_pct / 100.0);
        let swell_thresh = u_din * (config.swell_threshold_pct / 100.0);
        let int_thresh = u_din * (config.interrupt_threshold_pct / 100.0);
        let hyst = u_din * (config.hysteresis_pct / 100.0);

        let current_type = if urms_half < int_thresh {
            PqEventType::Interruption
        } else if urms_half < dip_thresh {
            PqEventType::Dip
        } else if urms_half > swell_thresh {
            PqEventType::Swell
        } else {
            PqEventType::None
        };

        if self.active_event.is_active {
            self.pending_count = 0;
            self.pending_type = PqEventType::None;
            // Event currently in progress
            let event_type = self.active_event.event_type;
            let ended = match event_type {
                PqEventType::Interruption => urms_half >= int_thresh + hyst,
                PqEventType::Dip => urms_half >= dip_thresh + hyst,
                PqEventType::Swell => urms_half <= swell_thresh - hyst,
                PqEventType::None => true,
            };

            if ended {
                // Event finished
                self.active_event.is_active = false;
                if now_ns >= self.active_event.start_timestamp_ns {
                    self.active_event.duration_ms =
                        (now_ns - self.active_event.start_timestamp_ns) as f32 / 1_000_000.0;
                }
                self.last_completed_event = self.active_event;
                self.active_event = PqEventRecord::default();
                return Some(self.last_completed_event);
            } else {
                // Update extremum during active event
                match event_type {
                    PqEventType::Dip | PqEventType::Interruption => {
                        self.active_event.extremum_v = self.active_event.extremum_v.min(urms_half);
                    }
                    PqEventType::Swell => {
                        self.active_event.extremum_v = self.active_event.extremum_v.max(urms_half);
                    }
                    _ => {}
                }
                if now_ns >= self.active_event.start_timestamp_ns {
                    self.active_event.duration_ms =
                        (now_ns - self.active_event.start_timestamp_ns) as f32 / 1_000_000.0;
                }
            }
        } else {
            // No active event, require min 3 half-cycles (30 ms) to activate and reject short switching spikes
            const MIN_HALF_CYCLES: u8 = 3;
            if current_type != PqEventType::None {
                if current_type == self.pending_type {
                    self.pending_count = self.pending_count.saturating_add(1);
                } else {
                    self.pending_type = current_type;
                    self.pending_count = 1;
                    self.pending_start_ns = now_ns;
                }

                if self.pending_count >= MIN_HALF_CYCLES {
                    self.active_event = PqEventRecord {
                        event_type: current_type,
                        phase_index,
                        start_timestamp_ns: self.pending_start_ns,
                        duration_ms: 0.0,
                        extremum_v: urms_half,
                        reference_v: u_din,
                        is_active: true,
                    };
                    self.pending_count = 0;
                    self.pending_type = PqEventType::None;

                    self.event_count += 1;
                    match current_type {
                        PqEventType::Dip => self.dip_count += 1,
                        PqEventType::Swell => self.swell_count += 1,
                        PqEventType::Interruption => self.interruption_count += 1,
                        _ => {}
                    }
                }
            } else {
                self.pending_count = 0;
                self.pending_type = PqEventType::None;
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies a dip event is detected and completed with the correct duration and extremum.
    #[test]
    fn test_dip_detection() {
        let mut detector = PowerQualityEventDetector::default();
        let config = PqEventConfig::default(); // 230.0 V, Dip < 207 V

        // Normal 230 V
        assert!(detector
            .process_half_cycle(0, 230.0, 1_000_000_000, &config)
            .is_none());
        assert!(!detector.active_event.is_active);

        // Dip occurs: 180 V (< 207 V)
        assert!(detector
            .process_half_cycle(0, 180.0, 1_010_000_000, &config)
            .is_none());
        assert!(detector.active_event.is_active);
        assert_eq!(detector.active_event.event_type, PqEventType::Dip);
        assert_eq!(detector.dip_count, 1);

        // Recovery: 235 V (>= 207 V + 2.3 V hysteresis)
        let event = detector.process_half_cycle(0, 235.0, 1_060_000_000, &config);
        assert!(event.is_some());
        let completed = event.unwrap();
        assert_eq!(completed.event_type, PqEventType::Dip);
        assert_eq!(completed.duration_ms, 50.0);
        assert_eq!(completed.extremum_v, 180.0);
    }

    /// Verifies a swell event is detected and completed with the correct duration and extremum.
    #[test]
    fn test_swell_detection() {
        let mut detector = PowerQualityEventDetector::default();
        let config = PqEventConfig::default(); // 230.0 V, Swell > 253 V

        // Normal
        assert!(detector
            .process_half_cycle(0, 230.0, 1_000_000_000, &config)
            .is_none());

        // Swell occurs: 270 V (> 253 V)
        assert!(detector
            .process_half_cycle(0, 270.0, 1_010_000_000, &config)
            .is_none());
        assert!(detector.active_event.is_active);
        assert_eq!(detector.active_event.event_type, PqEventType::Swell);
        assert_eq!(detector.swell_count, 1);

        // Recovery: 220 V (< 253 V - 2.3 V)
        let event = detector.process_half_cycle(0, 220.0, 1_110_000_000, &config);
        assert!(event.is_some());
        let completed = event.unwrap();
        assert_eq!(completed.event_type, PqEventType::Swell);
        assert_eq!(completed.duration_ms, 100.0);
        assert_eq!(completed.extremum_v, 270.0);
    }
}
