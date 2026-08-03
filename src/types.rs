//! Core data structures, configuration and measurement socket metrics.
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

use alloc::boxed::Box;
use crate::harmonics::{FftCache, InterharmonicAccumulator};
use crate::urms::UrmsHalfCycle;
use crate::flicker::FlickerMeter;
use serde::{Serialize, Deserialize};

pub const FREQ_NOMINAL_50: f32 = 50.0;
pub const FREQ_NOMINAL_60: f32 = 60.0;

pub const ADC_SAMPLES_50HZ_CYCLE: f32 = 160.0;
pub const ADC_SAMPLES_60HZ_CYCLE: usize = 133;

pub const NUMBER_HARMONICS: usize = 50;
pub const NUMBER_INTERHARMONICS: usize = 49;

/// Model that converts kernel time to UTC using a drift factor.
#[derive(Clone, Debug)]
pub struct TimeModel {
    pub utc_at_boot_ns: u64,
    pub ktime_at_boot_ns: u64,
    pub drift_factor: f32,
    pub last_calibration_ktime_ns: u64,
}

impl TimeModel {
    /// Converts a kernel timestamp to UTC using the current drift factor.
    ///
    /// # Arguments
    ///
    /// * `ktime_ns` - Kernel timestamp in nanoseconds.
    ///
    /// # Returns
    ///
    /// The equivalent UTC timestamp in nanoseconds.
    pub fn ktime_to_utc(&self, ktime_ns: u64) -> u64 {
        let delta_ktime = (ktime_ns as i64) - (self.ktime_at_boot_ns as i64);
        let delta_utc = (delta_ktime as f64) * (self.drift_factor as f64);
        (self.utc_at_boot_ns as i64 + delta_utc as i64) as u64
    }

    /// Creates a `TimeModel` from the current UTC and kernel times with drift factor 1.0.
    ///
    /// # Arguments
    ///
    /// * `utc_now_ns` - Current UTC timestamp in nanoseconds.
    /// * `ktime_now_ns` - Current kernel timestamp in nanoseconds.
    ///
    /// # Returns
    ///
    /// A new `TimeModel` initialized from the given timestamps.
    pub fn init_from_system(utc_now_ns: u64, ktime_now_ns: u64) -> Self {
        TimeModel {
            utc_at_boot_ns: utc_now_ns,
            ktime_at_boot_ns: ktime_now_ns,
            drift_factor: 1.0,
            last_calibration_ktime_ns: ktime_now_ns,
        }
    }

    /// Recomputes the drift factor from a new UTC/kernel time pair.
    ///
    /// # Arguments
    ///
    /// * `utc_new_ns` - New UTC timestamp in nanoseconds.
    /// * `ktime_new_ns` - New kernel timestamp in nanoseconds.
    pub fn recalibrate(&mut self, utc_new_ns: u64, ktime_new_ns: u64) {
        let delta_ktime = (ktime_new_ns as i64 - self.last_calibration_ktime_ns as i64) as f64;
        let delta_utc = (utc_new_ns as i64 - self.utc_at_boot_ns as i64) as f64;

        if delta_ktime.abs() > 1e6 {
            self.drift_factor = (delta_utc / delta_ktime) as f32;
            self.last_calibration_ktime_ns = ktime_new_ns;
        }
    }
}

impl Default for TimeModel {
    /// Returns a zeroed `TimeModel` with drift factor 1.0.
    fn default() -> Self {
        TimeModel {
            utc_at_boot_ns: 0,
            ktime_at_boot_ns: 0,
            drift_factor: 1.0,
            last_calibration_ktime_ns: 0,
        }
    }
}

/// Configuration parameters for the phase-locked loop.
#[derive(Clone, Debug)]
pub struct PllConfig {
    pub kp: f32,
    pub ki: f32,
    pub freq_min: f32,
    pub freq_max: f32,
    pub lock_threshold: f32,
    // Minimum signal amplitude for phase-error normalization (configurable per signal level)
    pub norm_threshold: f32,
    // Anti-windup clamp for the integrator (configurable per installation)
    pub integrator_clamp: f32,
    // EMA alpha for lock-error accumulator (configurable response speed)
    pub lock_ema_alpha: f32,
}

impl Default for PllConfig {
    /// Returns a `PllConfig` with the default PLL constants.
    fn default() -> Self {
        Self {
            kp: 0.002,
            ki: 0.00005,
            freq_min: 40.0,
            freq_max: 60.0,
            lock_threshold: 0.5,
            norm_threshold: crate::pll::PLL_NORM_THRESHOLD,
            integrator_clamp: crate::pll::PLL_INTEGRATOR_CLAMP,
            lock_ema_alpha: crate::pll::PLL_LOCK_EMA_ALPHA,
        }
    }
}

/// Top-level configuration for the metrology insight engine.
#[derive(Clone)]
pub struct MetrologyInsightConfig {
    pub avg_sec: f32,
    pub adc_samples_seconds: f32,
    pub adc_samples_per_cycle: f64,
    #[allow(dead_code)]
    pub num_harmonics: usize,
    pub calibration: CalibrationFactors,
    pub time_model: TimeModel,
    pub nominal_freq: f32,
    pub min_amplitude_voltage: f32,
    pub min_amplitude_current: f32,
    pub pll: PllConfig,
    pub event_config: crate::events::PqEventConfig,
    pub rvc_config: crate::rvc::RvcConfig,
    // Flicker meter configuration (IEC 61000-4-15 parameters)
    pub flicker: FlickerConfig,
    // Phase angle classification configuration
    pub phase: PhaseConfig,
    // Signal quality and processing thresholds
    pub signal: SignalConfig,
    // Standard electrical values per IEC 62053-21 §4
    pub standard_values: StandardElectricalValues,
}

impl Default for MetrologyInsightConfig {
    /// Returns a `MetrologyInsightConfig` with sensible defaults for a 50 Hz system.
    fn default() -> Self {
        Self {
            avg_sec: 0.0,
            adc_samples_seconds: 7812.5,
            adc_samples_per_cycle: 156.25,
            num_harmonics: NUMBER_HARMONICS,
            calibration: CalibrationFactors::default(),
            time_model: TimeModel::default(),
            nominal_freq: FREQ_NOMINAL_50,
            min_amplitude_voltage: 10.0,
            min_amplitude_current: 0.001,
            pll: PllConfig::default(),
            event_config: crate::events::PqEventConfig::default(),
            rvc_config: crate::rvc::RvcConfig::default(),
            flicker: FlickerConfig::default(),
            phase: PhaseConfig::default(),
            signal: SignalConfig::default(),
            standard_values: StandardElectricalValues::default(),
        }
    }
}

/// Flicker meter configuration (IEC 61000-4-15 Block 1-4 parameters).
#[derive(Clone, Debug)]
pub struct FlickerConfig {
    /// Nominal voltage used to seed avg_rms and as normalization reference (V)
    pub nominal_voltage: f32,
    /// Long-term RMS IIR filter time constant (seconds, ~1 min)
    pub rms_tc_seconds: f32,
    /// Block 4 smoothing time constant (seconds = 300 ms per IEC 61000-4-15)
    pub smooth_tc_seconds: f32,
    /// Minimum squared sample to seed avg_rms on first valid signal
    pub seed_threshold_sq: f32,
    /// Minimum RMS guard to prevent division by zero in Block 1 normalization
    pub min_rms_guard: f32,
    /// Minimum number of P_inst samples before Pst is computed
    pub pst_min_samples: u32,
}

impl Default for FlickerConfig {
    /// Returns a `FlickerConfig` with the IEC 61000-4-15 default parameters.
    fn default() -> Self {
        use crate::flicker::{
            FLICKER_RMS_TC_SECONDS, FLICKER_SMOOTH_TC_SECONDS,
            FLICKER_SEED_THRESHOLD_SQ, FLICKER_MIN_RMS_GUARD, FLICKER_PST_MIN_SAMPLES,
        };
        Self {
            nominal_voltage: 230.0,
            rms_tc_seconds: FLICKER_RMS_TC_SECONDS,
            smooth_tc_seconds: FLICKER_SMOOTH_TC_SECONDS,
            seed_threshold_sq: FLICKER_SEED_THRESHOLD_SQ,
            min_rms_guard: FLICKER_MIN_RMS_GUARD,
            pst_min_samples: FLICKER_PST_MIN_SAMPLES,
        }
    }
}

/// Phase angle direction classification configuration.
#[derive(Clone, Debug)]
pub struct PhaseConfig {
    /// Dead-band (degrees): angles within ±this value are classified as InPhase
    pub direction_deadband_deg: f32,
}

impl Default for PhaseConfig {
    /// Returns a `PhaseConfig` with the default direction dead-band.
    fn default() -> Self {
        Self {
            direction_deadband_deg: crate::phase::PHASE_DIRECTION_DEADBAND_DEG,
        }
    }
}

/// Signal quality and processing threshold configuration.
#[derive(Clone, Debug)]
pub struct SignalConfig {
    /// Minimum half-cycle fraction (of one cycle) required for a valid half-cycle RMS
    pub half_cycle_min_factor: f32,
    /// Minimum RMS before computing sync-resampled consistency error
    pub rms_consistency_min_guard: f32,
    /// error_accum threshold above which Q_FLAG_PLL_UNSETTLED is raised
    pub pll_error_accum_threshold: f32,
    /// Relative RMS/sync-RMS error threshold for Q_FLAG_SYNC_INCONSISTENT
    pub sync_consistency_threshold: f32,
}

impl Default for SignalConfig {
    /// Returns a `SignalConfig` with the default signal processing thresholds.
    fn default() -> Self {
        use crate::signal::{HALF_CYCLE_MIN_FACTOR, RMS_CONSISTENCY_MIN_GUARD, SYNC_CONSISTENCY_THRESHOLD};
        use crate::pll::PLL_ERROR_ACCUM_THRESHOLD;
        Self {
            half_cycle_min_factor: HALF_CYCLE_MIN_FACTOR,
            rms_consistency_min_guard: RMS_CONSISTENCY_MIN_GUARD,
            pll_error_accum_threshold: PLL_ERROR_ACCUM_THRESHOLD,
            sync_consistency_threshold: SYNC_CONSISTENCY_THRESHOLD,
        }
    }
}

/// Standard electrical values per IEC 62053-21 §4.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StandardElectricalValues {
    /// Nominal voltage (Un) — e.g. 230 V phase-to-neutral
    pub un_v: f32,
    /// Nominal current (In) — e.g. 5 A
    pub in_a: f32,
    /// Maximum current (Imax) — e.g. 10 A
    pub imax_a: f32,
    /// Minimum current (Imin) — 0.05 In per IEC 62053-21 Table 2
    pub imin_a: f32,
    /// Starting current (Ist) — 0.004 In direct / 0.002 In CT
    pub ist_a: f32,
    /// Nominal frequency (fn) — 50 or 60 Hz
    pub fn_hz: f32,
}

impl Default for StandardElectricalValues {
    /// Returns `StandardElectricalValues` for a 230 V / 5 A meter.
    fn default() -> Self {
        Self {
            un_v: 230.0,
            in_a: 5.0,
            imax_a: 10.0,
            imin_a: 0.02 * 5.0,     // 0.10 A (CT connection)
            ist_a: 0.002 * 5.0,      // 0.01 A (CT connection)
            fn_hz: crate::FREQ_NOMINAL_50,
        }
    }
}

/// Hardware calibration gains and offsets applied to raw ADC samples.
#[derive(Debug, Clone, Default)]
pub struct CalibrationFactors {
    pub v_gain: f32,
    pub i_gain: [f32; 3],
    pub phase_offset: [f32; 3],
    pub phase_delay_us: [f32; 3],
    pub temp_coeff: f64,
    pub v_lsb_to_phys: f32,
    pub i_lsb_to_phys: f32,
}

/// Electrical system wiring mode (single or three phase, with or without neutral).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SystemMode {
    #[default]
    SinglePhase,
    SinglePhaseN,
    ThreePhase3Wire,
    ThreePhase4Wire,
}

impl SystemMode {
    /// Returns the number of conductors (active phases) for this system mode.
    ///
    /// # Returns
    ///
    /// The number of active phase conductors (1 to 4).
    pub const fn active_phases(self) -> usize {
        match self {
            SystemMode::SinglePhase => 1,
            SystemMode::SinglePhaseN => 2,
            SystemMode::ThreePhase3Wire => 3,
            SystemMode::ThreePhase4Wire => 4,
        }
    }

    /// Returns whether this system mode includes a neutral conductor.
    ///
    /// # Returns
    ///
    /// `true` for single-phase+N and three-phase 4-wire modes.
    pub const fn has_neutral(self) -> bool {
        matches!(self, SystemMode::SinglePhaseN | SystemMode::ThreePhase4Wire)
    }
}

/// Per-phase measurement state (voltage, current, angles, power, flicker, events, RVC, interharmonics).
#[derive(Debug, Clone)]
pub struct PhaseData {
    pub voltage: MetrologyInsightSignal,
    pub current: MetrologyInsightSignal,
    pub phase_angles: PhaseAngleMetrics,
    pub power_metrics: PowerMetrics,
    pub flicker_meter: FlickerMeter,
    pub event_detector: crate::events::PowerQualityEventDetector,
    pub rvc_detector: crate::rvc::RvcDetector,
    pub interharm_acc: InterharmonicAccumulator,
}

impl Default for PhaseData {
    /// Returns a `PhaseData` with default voltage/current signals and detectors.
    fn default() -> Self {
        Self {
            voltage: MetrologyInsightSignal::default(),
            current: {
                let mut s = MetrologyInsightSignal::default();
                s.signal_type = MetrologyInsightSignalType::Current;
                s
            },
            phase_angles: PhaseAngleMetrics::default(),
            power_metrics: PowerMetrics::default(),
            flicker_meter: FlickerMeter::default(),
            event_detector: crate::events::PowerQualityEventDetector::default(),
            rvc_detector: crate::rvc::RvcDetector::default(),
            interharm_acc: InterharmonicAccumulator::new(
                crate::harmonics::FFT_RESOLUTION as f32 * crate::FREQ_NOMINAL_50,
            ),
        }
    }
}

/// Container holding per-phase data plus total power, energy and unbalance metrics.
#[derive(Debug, Clone)]
pub struct MetrologyInsightSocket {
    pub phases: [Box<PhaseData>; 4],
    pub power_metrics_total: PowerMetrics,
    pub energy_metrics: EnergyMetrics,
    pub unbalance_metrics: crate::unbalance::UnbalanceMetrics,
}

impl Default for MetrologyInsightSocket {
    /// Returns a `MetrologyInsightSocket` with four default phase slots.
    fn default() -> Self {
        Self {
            phases: [
                Box::new(PhaseData::default()),
                Box::new(PhaseData::default()),
                Box::new(PhaseData::default()),
                Box::new(PhaseData::default()),
            ],
            power_metrics_total: PowerMetrics::default(),
            energy_metrics: EnergyMetrics::default(),
            unbalance_metrics: crate::unbalance::UnbalanceMetrics::default(),
        }
    }
}

/// Main entry point holding the socket state, configuration and FFT cache.
pub struct MetrologyInsight {
    pub socket: Box<MetrologyInsightSocket>,
    pub config: MetrologyInsightConfig,
    pub fft_cache: Option<FftCache>,
    pub active_phases: usize,
}

impl MetrologyInsight {
    /// Creates a `MetrologyInsight` from the given config and applies it to all sub-components.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration to use for the new instance.
    ///
    /// # Returns
    ///
    /// A fully initialized `MetrologyInsight` instance.
    pub fn new(config: MetrologyInsightConfig) -> Self {
        let mut instance = Self {
            socket: Box::new(MetrologyInsightSocket::default()),
            config,
            fft_cache: None,
            active_phases: 1,
        };
        instance.apply_config();
        instance
    }

    /// Propagates the current `config` to all sub-components that hold internal state
    /// derived from configuration (e.g. FlickerMeter nominal voltage).
    /// Call this after modifying `self.config` at runtime.
    pub fn apply_config(&mut self) {
        let nominal_v = self.config.flicker.nominal_voltage;
        for phase in &mut self.socket.phases {
            phase.flicker_meter.set_nominal_voltage(nominal_v);
        }
    }

    /// Convenience setter that updates the nominal voltage in event_config, rvc_config and
    /// flicker config simultaneously, then propagates it via `apply_config()`.
    ///
    /// # Arguments
    ///
    /// * `voltage_v` - New nominal voltage in volts.
    pub fn set_nominal_voltage(&mut self, voltage_v: f32) {
        self.config.event_config.nominal_voltage = voltage_v;
        self.config.flicker.nominal_voltage = voltage_v;
        self.apply_config();
    }
}

/// Identifies whether a signal is a voltage or a current.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetrologyInsightSignalType {
    Voltage,
    Current,
}

impl MetrologyInsightSignalType {
    /// Returns the minimum amplitude required for this signal type from the given config.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration containing the voltage/current minimum amplitudes.
    ///
    /// # Returns
    ///
    /// The minimum amplitude threshold in volts or amperes.
    pub fn min_amplitude(&self, config: &MetrologyInsightConfig) -> f32 {
        match self {
            MetrologyInsightSignalType::Voltage => config.min_amplitude_voltage,
            MetrologyInsightSignalType::Current => config.min_amplitude_current,
        }
    }
}

impl Default for MetrologyInsightSignalType {
    /// Returns the default signal type, `Voltage`.
    fn default() -> Self {
        MetrologyInsightSignalType::Voltage
    }
}

pub const MAX_SIGNAL_SAMPLES: usize = 160;

/// Per-phase voltage or current measurement: waveform, RMS, harmonics, quality flags and PLL state.
#[derive(Clone, Copy, Debug)]
pub struct MetrologyInsightSignal {
    pub real_wave: [f32; MAX_SIGNAL_SAMPLES],
    pub real_wave_len: usize,
    pub length: usize,
    pub length_cycle: usize,
    pub calc_freq: bool,
    pub peak: f32,
    pub rms: f32,
    pub rms_10cycle: f32,
    pub cycle_10_sq_sum: f32,
    pub cycle_10_count: u8,
    pub freq_nominal: f32,
    pub freq_zc: f32,
    pub harmonics: [f32; NUMBER_HARMONICS],
    pub interharmonics: [f32; NUMBER_INTERHARMONICS],
    pub thd: f32,
    pub sc_thres: f32,
    pub signal_type: MetrologyInsightSignalType,
    pub adc_factor: f32,
    pub adc_scale: f32,
    pub dc_offset: f32,
    pub pll_state: PllState,
    pub quality_flags: u32,
    pub rms_sync: f32,
    pub consistency_error: f32,
    pub frame_start_ns: u64,
    pub urms_half_cycle: UrmsHalfCycle,
}

pub const Q_FLAG_OK: u32 = 0x0000;
pub const Q_FLAG_PLL_UNSETTLED: u32 = 0x0001;
pub const Q_FLAG_SYNC_INCONSISTENT: u32 = 0x0002;
pub const Q_FLAG_OUT_OF_RANGE: u32 = 0x0004;
pub const Q_FLAG_EVENT_MARKED: u32 = 0x0008;
/// Dummy helper retained to keep flag-related items alive.
fn _dummy_flags() {}

/// Phase-locked loop state (phase, estimated frequency, lock status).
#[derive(Clone, Copy, Debug, Default)]
pub struct PllState {
    pub phase: f32,
    pub freq_est: f32,
    pub freq_10s: f32,
    pub integrator: f32,
    pub locked: bool,
    pub error_accum: f32,
    pub freq_buf: [f32; 10],
    pub freq_buf_idx: usize,
    pub freq_buf_count: usize,
    pub cycle_freq_sum: f32,
    pub cycle_freq_count: usize,
}

impl MetrologyInsightSignal {
    /// Returns whether this signal is a current.
    ///
    /// # Returns
    ///
    /// `true` if the signal type is `Current`.
    pub fn is_current(&self) -> bool {
        matches!(self.signal_type, MetrologyInsightSignalType::Current)
    }

    /// Returns the valid portion of the captured real waveform as a slice.
    ///
    /// # Returns
    ///
    /// A slice of the stored samples up to `real_wave_len`.
    pub fn real_wave_slice(&self) -> &[f32] {
        &self.real_wave[..self.real_wave_len.min(MAX_SIGNAL_SAMPLES)]
    }

    /// Appends one ADC sample to the real waveform buffer if space is available.
    ///
    /// # Arguments
    ///
    /// * `val` - Sample value to append.
    pub fn push_real_sample(&mut self, val: f32) {
        if self.real_wave_len < MAX_SIGNAL_SAMPLES {
            self.real_wave[self.real_wave_len] = val;
            self.real_wave_len += 1;
        }
    }

    /// Clears the captured real waveform samples.
    pub fn clear_samples(&mut self) {
        self.real_wave_len = 0;
    }
}

impl Default for MetrologyInsightSignal {
    /// Returns a `MetrologyInsightSignal` with all fields zeroed and type `Voltage`.
    fn default() -> Self {
        Self {
            real_wave: [0.0; MAX_SIGNAL_SAMPLES],
            real_wave_len: 0,
            length: 0,
            length_cycle: 0,
            calc_freq: false,
            peak: 0.0,
            rms: 0.0,
            rms_10cycle: 0.0,
            cycle_10_sq_sum: 0.0,
            cycle_10_count: 0,
            freq_nominal: FREQ_NOMINAL_50,
            freq_zc: 0.0,
            harmonics: [0.0; NUMBER_HARMONICS],
            interharmonics: [0.0; NUMBER_INTERHARMONICS],
            thd: 0.0,
            sc_thres: 0.0,
            signal_type: MetrologyInsightSignalType::Voltage,
            adc_factor: 1.0,
            adc_scale: 1.0,
            dc_offset: 0.0,
            pll_state: PllState::default(),
            quality_flags: Q_FLAG_OK,
            rms_sync: 0.0,
            consistency_error: 0.0,
            frame_start_ns: 0,
            urms_half_cycle: UrmsHalfCycle::default(),
        }
    }
}

/// Classification of the phase angle between current and voltage.
#[derive(Debug, Clone)]
pub enum PhaseDirection {
    Inductive,
    Capacitive,
    InPhase,
}

impl Default for PhaseDirection {
    /// Returns the default phase direction, `InPhase`.
    fn default() -> Self {
        PhaseDirection::InPhase
    }
}

impl PhaseDirection {
    /// Returns a human-readable description of the phase direction.
    ///
    /// # Returns
    ///
    /// A static string describing the direction.
    pub fn as_str(&self) -> &'static str {
        match self {
            PhaseDirection::Inductive => "Inductive (current lags voltage)",
            PhaseDirection::Capacitive => "Capacitive (current leads voltage)",
            PhaseDirection::InPhase => "In phase (no phase difference)",
        }
    }
}

/// Phase angle measurements and direction classification for a phase.
#[derive(Debug, Clone, Default)]
pub struct PhaseAngleMetrics {
    pub c2v_angle: f32,
    pub v_angle: f32,
    pub c_angle: f32,
    pub direction: PhaseDirection,
}

impl PhaseAngleMetrics {
    /// Returns the description of the classified phase direction.
    ///
    /// # Returns
    ///
    /// A static string describing the direction.
    pub fn direction_description(&self) -> &'static str {
        self.direction.as_str()
    }
}

/// Real, reactive and apparent power plus power factor for a phase.
#[derive(Debug, Clone, Default)]
pub struct PowerMetrics {
    pub real_power: f32,
    pub reactive_power: f32,
    pub apparent_power: f32,
    pub power_factor: f32,
}

/// Active energy imported/exported, split into four quadrants.
#[derive(Debug, Clone, Default)]
pub struct ActiveEnergyMetrics {
    pub imported: f64,
    pub exported: f64,
    pub balance: f64,
    pub q1: f64,
    pub q2: f64,
    pub q3: f64,
    pub q4: f64,
    pub q1_uj: i128,
    pub q2_uj: i128,
    pub q3_uj: i128,
    pub q4_uj: i128,
}

impl ActiveEnergyMetrics {
    /// Returns the total imported active energy (quadrants Q1 + Q4).
    ///
    /// # Returns
    ///
    /// The imported energy in Wh.
    pub fn imported(&self) -> f64 {
        self.q1 + self.q4
    }

    /// Returns the total exported active energy (quadrants Q2 + Q3).
    ///
    /// # Returns
    ///
    /// The exported energy in Wh.
    pub fn exported(&self) -> f64 {
        self.q2 + self.q3
    }

    /// Returns the net active energy balance (imported minus exported).
    ///
    /// # Returns
    ///
    /// The net energy balance in Wh.
    pub fn balance(&self) -> f64 {
        self.imported() - self.exported()
    }
}

/// Reactive energy capacitive/inductive, split into four quadrants.
#[derive(Debug, Clone, Default)]
pub struct ReactiveEnergyMetrics {
    pub capacitive: f64,
    pub inductive: f64,
    pub balance: f64,
    pub q1: f64,
    pub q2: f64,
    pub q3: f64,
    pub q4: f64,
    pub q1_uj: i128,
    pub q2_uj: i128,
    pub q3_uj: i128,
    pub q4_uj: i128,
}

impl ReactiveEnergyMetrics {
    /// Returns the total inductive reactive energy (quadrants Q1 + Q3).
    ///
    /// # Returns
    ///
    /// The inductive energy in VARh.
    pub fn inductive(&self) -> f64 {
        self.q1 + self.q3
    }

    /// Returns the total capacitive reactive energy (quadrants Q2 + Q4).
    ///
    /// # Returns
    ///
    /// The capacitive energy in VARh.
    pub fn capacitive(&self) -> f64 {
        self.q2 + self.q4
    }

    /// Returns the net reactive energy balance (inductive minus capacitive).
    ///
    /// # Returns
    ///
    /// The net reactive energy balance in VARh.
    pub fn balance(&self) -> f64 {
        (self.q1 + self.q2) - (self.q3 + self.q4)
    }
}

/// Aggregate active and reactive energy metrics.
#[derive(Debug, Clone, Default)]
pub struct EnergyMetrics {
    pub active: ActiveEnergyMetrics,
    pub reactive: ReactiveEnergyMetrics,
}

/// Fixed 256-byte power quality aggregation record for periodic storage.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct PqAggregationRecord {
    pub timestamp_ms: u64,
    pub aggregation_type: u8, // 0 = 3s, 1 = 10min, 2 = 2h
    // Voltages, currents, frequency
    pub v_rms: [f32; 3], 
    pub i_rms: [f32; 3], 
    pub frequency: f32,
    pub v_peak: [f32; 3], 
    pub i_peak: [f32; 3],
    // Total power and energy
    pub active_power: f32, 
    pub reactive_power: f32, 
    pub apparent_power: f32, 
    pub power_factor: f32,
    pub active_energy_imp: f32, 
    pub active_energy_exp: f32,
    // Flicker P_inst
    pub flicker: [f32; 3],
    // EN 61000-4-30 additions
    pub flicker_pst: [f32; 3],
    pub v_rms_10cycle: [f32; 3],
    pub freq_10s: f32,
    pub u2_unbalance: f32,
    pub u0_unbalance: f32,
    // Current unbalance (§5.13.6)
    pub u2_i_ratio_pct: f32,
    pub u0_i_ratio_pct: f32,
    pub i0_zero_seq: f32,
    pub i1_pos_seq: f32,
    pub i2_neg_seq: f32,
    // Quality indices — max/min per window (§B.4)
    pub v_rms_min: [f32; 3],
    pub v_rms_max: [f32; 3],
    pub freq_min: f32,
    pub freq_max: f32,
    // Event counts — cumulative (per-window delta computed by consumer)
    pub dip_count: u32,
    pub swell_count: u32,
    pub interrupt_count: u32,
    pub rvc_count: u32,
    // Main harmonics (THD of each phase)
    pub v_thd: [f32; 3], 
    pub i_thd: [f32; 3],
    // Window validity tracking (§4.5.2) — number of clean/total 10/12-cycle windows
    pub clean_windows: u16,
    pub total_windows: u16,
    // RVC metrics (§5.11) — max ΔUmax and ΔUss per phase in interval
    pub rvc_delta_u_max_pct: [f32; 3],
    pub rvc_delta_u_ss_pct: [f32; 3],
    // Padding reserved to reach exactly 256 bytes (flash sector alignment)
    // 253 bytes base + 3 bytes padding = 256 bytes.
    #[serde(skip)]
    pub padding: [u8; 3], 
}

impl PqAggregationRecord {
    /// Returns a `PqAggregationRecord` with all fields zeroed.
    ///
    /// # Returns
    ///
    /// An empty aggregation record.
    pub fn empty() -> Self {
        Self {
            timestamp_ms: 0,
            aggregation_type: 0,
            v_rms: [0.0; 3],
            i_rms: [0.0; 3],
            frequency: 0.0,
            v_peak: [0.0; 3],
            i_peak: [0.0; 3],
            active_power: 0.0,
            reactive_power: 0.0,
            apparent_power: 0.0,
            power_factor: 0.0,
            active_energy_imp: 0.0,
            active_energy_exp: 0.0,
            flicker: [0.0; 3],
            flicker_pst: [0.0; 3],
            v_rms_10cycle: [0.0; 3],
            freq_10s: 0.0,
            u2_unbalance: 0.0,
            u0_unbalance: 0.0,
            u2_i_ratio_pct: 0.0,
            u0_i_ratio_pct: 0.0,
            i0_zero_seq: 0.0,
            i1_pos_seq: 0.0,
            i2_neg_seq: 0.0,
            v_rms_min: [f32::MAX; 3],
            v_rms_max: [f32::MIN; 3],
            freq_min: f32::MAX,
            freq_max: f32::MIN,
            dip_count: 0,
            swell_count: 0,
            interrupt_count: 0,
            rvc_count: 0,
            v_thd: [0.0; 3],
            i_thd: [0.0; 3],
            clean_windows: 0,
            total_windows: 0,
            rvc_delta_u_max_pct: [0.0; 3],
            rvc_delta_u_ss_pct: [0.0; 3],
            padding: [0u8; 3],
        }
    }

    /// Serializes the record into a fixed 256-byte little-endian buffer.
    ///
    /// # Returns
    ///
    /// A 256-byte array with all fields packed in a fixed layout.
    pub fn to_bytes(&self) -> [u8; 256] {
        let mut buf = [0u8; 256];
        let mut off = 0usize;

        /// Writes a u64 in little-endian order and advances the offset.
        fn write_u64(buf: &mut [u8; 256], off: &mut usize, v: u64) {
            buf[*off..*off + 8].copy_from_slice(&v.to_le_bytes());
            *off += 8;
        }
        /// Writes a u8 and advances the offset.
        fn write_u8(buf: &mut [u8; 256], off: &mut usize, v: u8) {
            buf[*off] = v;
            *off += 1;
        }
        /// Writes an f32 in little-endian order and advances the offset.
        fn write_f32(buf: &mut [u8; 256], off: &mut usize, v: f32) {
            buf[*off..*off + 4].copy_from_slice(&v.to_le_bytes());
            *off += 4;
        }
        /// Writes three f32 values (e.g. one per phase) and advances the offset.
        fn write_f32x3(buf: &mut [u8; 256], off: &mut usize, v: &[f32; 3]) {
            write_f32(buf, off, v[0]);
            write_f32(buf, off, v[1]);
            write_f32(buf, off, v[2]);
        }
        /// Writes a u32 in little-endian order and advances the offset.
        fn write_u32(buf: &mut [u8; 256], off: &mut usize, v: u32) {
            buf[*off..*off + 4].copy_from_slice(&v.to_le_bytes());
            *off += 4;
        }
        /// Writes a u16 in little-endian order and advances the offset.
        fn write_u16(buf: &mut [u8; 256], off: &mut usize, v: u16) {
            buf[*off..*off + 2].copy_from_slice(&v.to_le_bytes());
            *off += 2;
        }

        write_u64(&mut buf, &mut off, self.timestamp_ms);
        write_u8(&mut buf, &mut off, self.aggregation_type);
        write_f32x3(&mut buf, &mut off, &self.v_rms);
        write_f32x3(&mut buf, &mut off, &self.i_rms);
        write_f32(&mut buf, &mut off, self.frequency);
        write_f32x3(&mut buf, &mut off, &self.v_peak);
        write_f32x3(&mut buf, &mut off, &self.i_peak);
        write_f32(&mut buf, &mut off, self.active_power);
        write_f32(&mut buf, &mut off, self.reactive_power);
        write_f32(&mut buf, &mut off, self.apparent_power);
        write_f32(&mut buf, &mut off, self.power_factor);
        write_f32(&mut buf, &mut off, self.active_energy_imp);
        write_f32(&mut buf, &mut off, self.active_energy_exp);
        write_f32x3(&mut buf, &mut off, &self.flicker);
        write_f32x3(&mut buf, &mut off, &self.flicker_pst);
        write_f32x3(&mut buf, &mut off, &self.v_rms_10cycle);
        write_f32(&mut buf, &mut off, self.freq_10s);
        write_f32(&mut buf, &mut off, self.u2_unbalance);
        write_f32(&mut buf, &mut off, self.u0_unbalance);
        write_f32(&mut buf, &mut off, self.u2_i_ratio_pct);
        write_f32(&mut buf, &mut off, self.u0_i_ratio_pct);
        write_f32(&mut buf, &mut off, self.i0_zero_seq);
        write_f32(&mut buf, &mut off, self.i1_pos_seq);
        write_f32(&mut buf, &mut off, self.i2_neg_seq);
        write_f32x3(&mut buf, &mut off, &self.v_rms_min);
        write_f32x3(&mut buf, &mut off, &self.v_rms_max);
        write_f32(&mut buf, &mut off, self.freq_min);
        write_f32(&mut buf, &mut off, self.freq_max);
        write_u32(&mut buf, &mut off, self.dip_count);
        write_u32(&mut buf, &mut off, self.swell_count);
        write_u32(&mut buf, &mut off, self.interrupt_count);
        write_u32(&mut buf, &mut off, self.rvc_count);
        write_f32x3(&mut buf, &mut off, &self.v_thd);
        write_f32x3(&mut buf, &mut off, &self.i_thd);
        write_u16(&mut buf, &mut off, self.clean_windows);
        write_u16(&mut buf, &mut off, self.total_windows);
        write_f32x3(&mut buf, &mut off, &self.rvc_delta_u_max_pct);
        write_f32x3(&mut buf, &mut off, &self.rvc_delta_u_ss_pct);
        // padding at offset 253, 3 bytes remain
        buf[off..].copy_from_slice(&self.padding);

        buf
    }

    /// Deserializes a record from a fixed 256-byte little-endian buffer.
    ///
    /// # Arguments
    ///
    /// * `bytes` - The 256-byte buffer produced by `to_bytes`.
    ///
    /// # Returns
    ///
    /// The reconstructed `PqAggregationRecord`.
    pub fn from_bytes(bytes: &[u8; 256]) -> Self {
        let mut off = 0usize;

        /// Reads a u64 in little-endian order and advances the offset.
        fn read_u64(buf: &[u8; 256], off: &mut usize) -> u64 {
            let v = u64::from_le_bytes(buf[*off..*off + 8].try_into().unwrap());
            *off += 8;
            v
        }
        /// Reads a u8 and advances the offset.
        fn read_u8(buf: &[u8; 256], off: &mut usize) -> u8 {
            let v = buf[*off];
            *off += 1;
            v
        }
        /// Reads an f32 in little-endian order and advances the offset.
        fn read_f32(buf: &[u8; 256], off: &mut usize) -> f32 {
            let v = f32::from_le_bytes(buf[*off..*off + 4].try_into().unwrap());
            *off += 4;
            v
        }
        /// Reads three f32 values (e.g. one per phase) and advances the offset.
        fn read_f32x3(buf: &[u8; 256], off: &mut usize) -> [f32; 3] {
            [read_f32(buf, off), read_f32(buf, off), read_f32(buf, off)]
        }
        /// Reads a u32 in little-endian order and advances the offset.
        fn read_u32(buf: &[u8; 256], off: &mut usize) -> u32 {
            let v = u32::from_le_bytes(buf[*off..*off + 4].try_into().unwrap());
            *off += 4;
            v
        }
        /// Reads a u16 in little-endian order and advances the offset.
        fn read_u16(buf: &[u8; 256], off: &mut usize) -> u16 {
            let v = u16::from_le_bytes(buf[*off..*off + 2].try_into().unwrap());
            *off += 2;
            v
        }

        Self {
            timestamp_ms: read_u64(bytes, &mut off),
            aggregation_type: read_u8(bytes, &mut off),
            v_rms: read_f32x3(bytes, &mut off),
            i_rms: read_f32x3(bytes, &mut off),
            frequency: read_f32(bytes, &mut off),
            v_peak: read_f32x3(bytes, &mut off),
            i_peak: read_f32x3(bytes, &mut off),
            active_power: read_f32(bytes, &mut off),
            reactive_power: read_f32(bytes, &mut off),
            apparent_power: read_f32(bytes, &mut off),
            power_factor: read_f32(bytes, &mut off),
            active_energy_imp: read_f32(bytes, &mut off),
            active_energy_exp: read_f32(bytes, &mut off),
            flicker: read_f32x3(bytes, &mut off),
            flicker_pst: read_f32x3(bytes, &mut off),
            v_rms_10cycle: read_f32x3(bytes, &mut off),
            freq_10s: read_f32(bytes, &mut off),
            u2_unbalance: read_f32(bytes, &mut off),
            u0_unbalance: read_f32(bytes, &mut off),
            u2_i_ratio_pct: read_f32(bytes, &mut off),
            u0_i_ratio_pct: read_f32(bytes, &mut off),
            i0_zero_seq: read_f32(bytes, &mut off),
            i1_pos_seq: read_f32(bytes, &mut off),
            i2_neg_seq: read_f32(bytes, &mut off),
            v_rms_min: read_f32x3(bytes, &mut off),
            v_rms_max: read_f32x3(bytes, &mut off),
            freq_min: read_f32(bytes, &mut off),
            freq_max: read_f32(bytes, &mut off),
            dip_count: read_u32(bytes, &mut off),
            swell_count: read_u32(bytes, &mut off),
            interrupt_count: read_u32(bytes, &mut off),
            rvc_count: read_u32(bytes, &mut off),
            v_thd: read_f32x3(bytes, &mut off),
            i_thd: read_f32x3(bytes, &mut off),
            clean_windows: read_u16(bytes, &mut off),
            total_windows: read_u16(bytes, &mut off),
            rvc_delta_u_max_pct: read_f32x3(bytes, &mut off),
            rvc_delta_u_ss_pct: read_f32x3(bytes, &mut off),
            padding: {
                let mut p = [0u8; 3];
                p.copy_from_slice(&bytes[off..off + 3]);
                p
            },
        }
    }
}
