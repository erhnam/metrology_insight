//! Physical ADC channel types and channel-to-phase mapping.
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

/// Physical ADC channel type (voltage or current per phase).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ChannelType {
    VoltageA = 0,
    CurrentA = 1,
    VoltageB = 2,
    CurrentB = 3,
    VoltageC = 4,
    CurrentC = 5,
    VoltageN = 6,
    CurrentN = 7,
}

/// Logical phase identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    A,
    B,
    C,
    Neutral,
}

/// A voltage–current pair belonging to the same phase.
#[derive(Debug, Clone, Copy)]
pub struct PhasePair {
    /// ADC channel index for voltage (0–7).
    pub voltage_channel: usize,
    /// ADC channel index for current (0–7).
    pub current_channel: usize,
    /// The logical phase this pair belongs to.
    pub phase: Phase,
}

/// Return the active phase pairs for a given system mode.
///
/// * `SinglePhase` → [PhaseA]
/// * `SinglePhaseN` → [PhaseA, Neutral]
/// * `ThreePhase3Wire` → [PhaseA, PhaseB, PhaseC]
/// * `ThreePhase4Wire` → [PhaseA, PhaseB, PhaseC, Neutral]
///
/// # Arguments
///
/// * `mode` — System wiring mode.
///
/// # Returns
///
/// The fixed array of active phase pairs for the given mode.
pub fn phase_pairs_for_mode(mode: crate::types::SystemMode) -> &'static [PhasePair; 4] {
    match mode {
        crate::types::SystemMode::SinglePhase
        | crate::types::SystemMode::SinglePhaseN
        | crate::types::SystemMode::ThreePhase3Wire
        | crate::types::SystemMode::ThreePhase4Wire => &DEFAULT_PAIRS,
    }
}

static DEFAULT_PAIRS: [PhasePair; 4] = [
    PhasePair { voltage_channel: 0, current_channel: 1, phase: Phase::A },
    PhasePair { voltage_channel: 2, current_channel: 3, phase: Phase::B },
    PhasePair { voltage_channel: 4, current_channel: 5, phase: Phase::C },
    PhasePair { voltage_channel: 7, current_channel: 6, phase: Phase::Neutral },
];

/// Signal polarity inversion flags for a phase pair.
///
/// Set when a transformer or CT is wired with reversed polarity.
#[derive(Debug, Clone, Copy)]
pub struct SignalInversion {
    pub invert_voltage: bool,
    pub invert_current: bool,
}

/// Default 3-phase + neutral channel map for ADS131M08.
///
/// CH0=V1, CH1=I1, CH2=V2, CH3=I2, CH4=V3, CH5=I3, CH6=VN, CH7=IN
pub const DEFAULT_CHANNEL_MAP: [ChannelType; 8] = [
    ChannelType::VoltageA,
    ChannelType::CurrentA,
    ChannelType::VoltageB,
    ChannelType::CurrentB,
    ChannelType::VoltageC,
    ChannelType::CurrentC,
    ChannelType::VoltageN,
    ChannelType::CurrentN,
];

/// Return the voltage–current phase pairs for the default 3-phase + neutral map.
///
/// # Returns
///
/// A copy of the default 3-phase + neutral [`PhasePair`] array.
pub fn default_phase_pairs() -> [PhasePair; 4] {
    DEFAULT_PAIRS
}

/// Group 8 channels into phase pairs using a channel-type map.
///
/// # Arguments
///
/// * `map` — Channel-type map for the 8 ADC channels.
///
/// # Returns
///
/// The four [`PhasePair`]s built from the map, falling back to default channel
/// indices when a channel type is not present.
pub fn channel_map_to_pairs(map: &[ChannelType; 8]) -> [PhasePair; 4] {
    /// Find the index of a channel type within the channel map.
    ///
    /// # Arguments
    ///
    /// * `map` — The 8-channel channel-type map.
    /// * `needle` — Channel type to locate.
    ///
    /// # Returns
    ///
    /// The matching channel index, or [`None`] when the channel is not present.
    fn find_ch(map: &[ChannelType; 8], needle: ChannelType) -> Option<usize> {
        map.iter().position(|&c| c == needle)
    }

    [
        PhasePair {
            voltage_channel: find_ch(map, ChannelType::VoltageA).unwrap_or(0),
            current_channel: find_ch(map, ChannelType::CurrentA).unwrap_or(1),
            phase: Phase::A,
        },
        PhasePair {
            voltage_channel: find_ch(map, ChannelType::VoltageB).unwrap_or(2),
            current_channel: find_ch(map, ChannelType::CurrentB).unwrap_or(3),
            phase: Phase::B,
        },
        PhasePair {
            voltage_channel: find_ch(map, ChannelType::VoltageC).unwrap_or(4),
            current_channel: find_ch(map, ChannelType::CurrentC).unwrap_or(5),
            phase: Phase::C,
        },
        PhasePair {
            voltage_channel: find_ch(map, ChannelType::VoltageN).unwrap_or(6),
            current_channel: find_ch(map, ChannelType::CurrentN).unwrap_or(7),
            phase: Phase::Neutral,
        },
    ]
}
