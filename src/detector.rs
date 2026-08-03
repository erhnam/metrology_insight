// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

//! Alarm Detector library — Rust port of the original C detector library.
//!
//! Monitors any metrological value (RMS, THD, frequency, power, energy…) and
//! drives a stateful ON/OFF alarm with configurable threshold, hysteresis and
//! debounce.

use crate::types::{MetrologyInsightSocket, PhaseData};

// ─── Operation ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Value,
    Abs,
    Gradient,
    AbsGradient,
}

impl Operation {
    /// Maps an operation name string to the corresponding `Operation`.
    ///
    /// # Arguments
    ///
    /// * `s` - String such as "absolute", "gradient" or "absGrad".
    ///
    /// # Returns
    ///
    /// The matching `Operation`, or `Operation::Value` for unknown strings.
    pub fn parse_str(s: &str) -> Self {
        match s {
            "absolute" => Operation::Abs,
            "gradient" => Operation::Gradient,
            "absGrad" => Operation::AbsGradient,
            _ => Operation::Value,
        }
    }
}

// ─── Group / Element ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Voltage,
    Current,
    Power,
    ActiveEnergy,
    ReactiveEnergy,
}

impl Group {
    /// Maps a group name string to the corresponding `Group`.
    ///
    /// # Arguments
    ///
    /// * `s` - String such as "voltage", "current", "power" or "a_energy".
    ///
    /// # Returns
    ///
    /// The matching `Group`, or `None` if the string is unknown.
    pub fn parse_str(s: &str) -> Option<Self> {
        match s {
            "voltage" => Some(Group::Voltage),
            "current" => Some(Group::Current),
            "power" => Some(Group::Power),
            "a_energy" => Some(Group::ActiveEnergy),
            "r_energy" => Some(Group::ReactiveEnergy),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Element {
    Rms,
    UrmsHalfCycle,
    Frequency,
    Thd,
    Instant,
    Phi,
    Active,
    Reactive,
    Apparent,
    Imported,
    Exported,
    Inductive,
    Capacitive,
}

impl Element {
    /// Maps an element name string to the corresponding `Element`.
    ///
    /// # Arguments
    ///
    /// * `s` - String such as "TRMS", "Frequency" or "THD".
    ///
    /// # Returns
    ///
    /// The matching `Element`, or `None` if the string is unknown.
    pub fn parse_str(s: &str) -> Option<Self> {
        match s {
            "TRMS" => Some(Element::Rms),
            "UrmsHalfCycle" => Some(Element::UrmsHalfCycle),
            "Frequency" => Some(Element::Frequency),
            "THD" => Some(Element::Thd),
            "instant" => Some(Element::Instant),
            "Phi" => Some(Element::Phi),
            "Active" => Some(Element::Active),
            "Reactive" => Some(Element::Reactive),
            "Apparent" => Some(Element::Apparent),
            "Imported" => Some(Element::Imported),
            "Exported" => Some(Element::Exported),
            "Inductive" => Some(Element::Inductive),
            "Capacitive" => Some(Element::Capacitive),
            _ => None,
        }
    }
}

// ─── Condition ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Condition {
    Gt,
    GtEq,
    Lt,
    LtEq,
    Eq,
    NotEq,
}

impl Condition {
    /// Checks whether `value` satisfies this condition against `threshold`.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to evaluate.
    /// * `threshold` - The threshold to compare against.
    ///
    /// # Returns
    ///
    /// `true` if the comparison holds.
    fn check(self, value: f32, threshold: f32) -> bool {
        match self {
            Condition::Gt => value > threshold,
            Condition::GtEq => value >= threshold,
            Condition::Lt => value < threshold,
            Condition::LtEq => value <= threshold,
            Condition::Eq => (value - threshold).abs() < f32::EPSILON,
            Condition::NotEq => (value - threshold).abs() >= f32::EPSILON,
        }
    }

    /// Maps a condition name string to the corresponding `Condition`.
    ///
    /// # Arguments
    ///
    /// * `s` - String such as "gt", "lt_eq" or "equal".
    ///
    /// # Returns
    ///
    /// The matching `Condition`, or `None` if the string is unknown.
    pub fn parse_str(s: &str) -> Option<Self> {
        match s {
            "gt" => Some(Condition::Gt),
            "gt_eq" => Some(Condition::GtEq),
            "lt" => Some(Condition::Lt),
            "lt_eq" => Some(Condition::LtEq),
            "equal" => Some(Condition::Eq),
            "not_equal" => Some(Condition::NotEq),
            _ => None,
        }
    }
}

// ─── Status ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    Off,
    On,
}

// ─── Detector ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Detector {
    pub condition: Condition,
    pub status: Status,

    pub th: f32,
    pub hyst: f32,

    pub threshold_on: f32,
    pub threshold_off: f32,

    pub debounce_on: u16,
    pub debounce_off: u16,
    debounce: u16,

    prev_raw: f32,
}

impl Detector {
    /// Creates a new `Detector`, interpreting `hyst_abs` as the absolute value of the
    /// hysteresis (e.g. 2.0 V or 0.1 Hz passed directly from the JSON).
    ///
    /// # Arguments
    ///
    /// * `condition` - Comparison condition for the alarm.
    /// * `th` - Alarm threshold.
    /// * `hyst_abs` - Absolute hysteresis band used to derive the OFF threshold.
    /// * `debounce` - Debounce count applied in both directions.
    ///
    /// # Returns
    ///
    /// A `Detector` initialized in the `Off` state.
    pub fn new(condition: Condition, th: f32, hyst_abs: f32, debounce: u16) -> Self {
        let h = hyst_abs;

        // For Gt: turns on when exceeding `th`, turns off when dropping below `th - h`
        // For Lt: turns on when dropping below `th`, turns off when rising above `th + h`
        let (threshold_on, threshold_off) = match condition {
            Condition::Gt | Condition::GtEq => (th, th - h),
            Condition::Lt | Condition::LtEq => (th, th + h),
            Condition::Eq | Condition::NotEq => (th + h, th - h),
        };

        Self {
            condition,
            status: Status::Off,
            th,
            hyst: hyst_abs,
            threshold_on,
            threshold_off,
            debounce_on: debounce,
            debounce_off: debounce,
            debounce,
            prev_raw: 0.0,
        }
    }

    /// Processes a raw value using the `Value` operation and optionally updates the status.
    ///
    /// # Arguments
    ///
    /// * `raw_value` - Raw metrological value to evaluate.
    /// * `update_status` - Whether the internal status may change to `On`.
    ///
    /// # Returns
    ///
    /// A tuple of whether a transition occurred and the current `Status`.
    pub fn process(&mut self, raw_value: f32, update_status: bool) -> (bool, Status) {
        self.process_with_op(raw_value, Operation::Value, update_status)
    }

    /// Processes a raw value with the given operation and optionally updates the status.
    ///
    /// # Arguments
    ///
    /// * `raw_value` - Raw metrological value to evaluate.
    /// * `op` - Operation applied to the raw value before comparing.
    /// * `update_status` - Whether the internal status may change to `On`.
    ///
    /// # Returns
    ///
    /// A tuple of whether a transition occurred and the current `Status`.
    pub fn process_with_op(
        &mut self,
        raw_value: f32,
        op: Operation,
        update_status: bool,
    ) -> (bool, Status) {
        let value = match op {
            Operation::Value => raw_value,
            Operation::Abs => raw_value.abs(),
            Operation::Gradient => raw_value - self.prev_raw,
            Operation::AbsGradient => (raw_value - self.prev_raw).abs(),
        };
        self.prev_raw = raw_value;

        let mut transition = false;

        match self.status {
            Status::On => {
                // To turn the alarm off, the recovery (normality) condition is evaluated:
                // - Overvoltage (Gt): turns off when the value drops back to or below the OFF threshold.
                // - Undervoltage (Lt): turns off when the value rises back to or above the OFF threshold.
                let is_normal = match self.condition {
                    Condition::Gt | Condition::GtEq => value <= self.threshold_off,
                    Condition::Lt | Condition::LtEq => value >= self.threshold_off,
                    Condition::Eq => (value - self.th).abs() > self.hyst,
                    Condition::NotEq => (value - self.th).abs() <= self.hyst,
                };

                if self.debounce > 0 {
                    if is_normal {
                        self.debounce -= 1;
                    } else {
                        self.debounce = self.debounce_off;
                    }
                }

                if is_normal && self.debounce == 0 {
                    self.status = Status::Off;
                    self.debounce = self.debounce_on;
                    transition = true;
                }
            }
            Status::Off => {
                let is_alarm = self.condition.check(value, self.threshold_on);

                if self.debounce > 0 {
                    if is_alarm {
                        self.debounce -= 1;
                    } else {
                        self.debounce = self.debounce_off;
                    }
                }

                if is_alarm && self.debounce == 0 {
                    if update_status {
                        self.status = Status::On;
                    }
                    self.debounce = self.debounce_off;
                    transition = true;
                }
            }
        }

        (transition, self.status)
    }

    /// Resets the detector to the `Off` state and clears the stored raw value.
    pub fn reset(&mut self) {
        self.status = Status::Off;
        self.debounce = self.debounce_on;
        self.prev_raw = 0.0;
    }
}

// ─── Value Extractor ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct ValueKey {
    pub phase: usize,
    pub group: Group,
    pub element: Element,
}

/// Extracts the metrological value addressed by `key` from the socket.
///
/// # Arguments
///
/// * `socket` - Socket holding the measured phase and energy data.
/// * `key` - Phase, group and element to read.
///
/// # Returns
///
/// The extracted value as `f32`, or `None` if the phase is out of range or the
/// group/element combination is not supported.
pub fn extract_value(socket: &MetrologyInsightSocket, key: ValueKey) -> Option<f32> {
    let phase: &PhaseData = socket.phases.get(key.phase)?;

    match (key.group, key.element) {
        (Group::Voltage, Element::Rms) => Some(phase.voltage.rms),
        (Group::Voltage, Element::UrmsHalfCycle) => Some(phase.voltage.urms_half_cycle.urms),
        (Group::Voltage, Element::Frequency) => Some(phase.voltage.pll_state.freq_est),
        (Group::Voltage, Element::Thd) => Some(phase.voltage.thd),

        (Group::Current, Element::Rms) => Some(phase.current.rms),
        (Group::Current, Element::UrmsHalfCycle) => Some(phase.current.urms_half_cycle.urms),
        (Group::Current, Element::Thd) => Some(phase.current.thd),
        (Group::Current, Element::Phi) => Some(phase.phase_angles.c2v_angle),

        (Group::Power, Element::Active) => Some(phase.power_metrics.real_power),
        (Group::Power, Element::Reactive) => Some(phase.power_metrics.reactive_power),
        (Group::Power, Element::Apparent) => Some(phase.power_metrics.apparent_power),

        (Group::ActiveEnergy, Element::Imported) => {
            Some(socket.energy_metrics.active.imported() as f32)
        }
        (Group::ActiveEnergy, Element::Exported) => {
            Some(socket.energy_metrics.active.exported() as f32)
        }

        (Group::ReactiveEnergy, Element::Inductive) => {
            Some(socket.energy_metrics.reactive.inductive() as f32)
        }
        (Group::ReactiveEnergy, Element::Capacitive) => {
            Some(socket.energy_metrics.reactive.capacitive() as f32)
        }

        _ => None,
    }
}

// ─── Detector Manager ──────────────────────────────────────────────────────────

pub const DETECTOR_MAX: usize = 50;

pub struct DetectorManager {
    slots: [Option<(ValueKey, Operation, Detector)>; DETECTOR_MAX],
}

impl DetectorManager {
    /// Creates a `DetectorManager` with no detectors allocated.
    ///
    /// # Returns
    ///
    /// A manager with all slots empty.
    pub fn new() -> Self {
        Self {
            slots: core::array::from_fn(|_| None),
        }
    }

    /// Allocates a new detector in the first free slot.
    ///
    /// # Arguments
    ///
    /// * `key` - Value key the detector monitors.
    /// * `op` - Operation applied to the raw value.
    /// * `condition` - Alarm comparison condition.
    /// * `th` - Alarm threshold.
    /// * `hyst_abs` - Absolute hysteresis band.
    /// * `debounce` - Debounce count in both directions.
    ///
    /// # Returns
    ///
    /// The slot index of the new detector, or `None` if all slots are occupied.
    pub fn create(
        &mut self,
        key: ValueKey,
        op: Operation,
        condition: Condition,
        th: f32,
        hyst_abs: f32,
        debounce: u16,
    ) -> Option<usize> {
        let slot = self.slots.iter().position(|s| s.is_none())?;
        self.slots[slot] = Some((key, op, Detector::new(condition, th, hyst_abs, debounce)));
        Some(slot)
    }

    /// Frees the detector at `id`, if it is a valid slot index.
    ///
    /// # Arguments
    ///
    /// * `id` - Slot index to free.
    pub fn delete(&mut self, id: usize) {
        if id < DETECTOR_MAX {
            self.slots[id] = None;
        }
    }

    /// Evaluates all detectors against the current socket values and invokes `on_event`
    /// on every status transition.
    ///
    /// # Arguments
    ///
    /// * `socket` - Socket with the current measured values.
    /// * `on_event` - Callback invoked with the detector id and new `Status` on transition.
    pub fn evaluate<F>(&mut self, socket: &MetrologyInsightSocket, mut on_event: F)
    where
        F: FnMut(usize, Status),
    {
        for (id, slot) in self.slots.iter_mut().enumerate() {
            if let Some((key, op, detector)) = slot {
                if let Some(raw) = extract_value(socket, *key) {
                    let (transition, status) = detector.process_with_op(raw, *op, true);
                    if transition {
                        on_event(id, status);
                    }
                }
            }
        }
    }
}

impl Default for DetectorManager {
    /// Returns an empty `DetectorManager` via [`Self::new`].
    fn default() -> Self {
        Self::new()
    }
}
