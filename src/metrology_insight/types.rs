pub const FREQ_NOMINAL_50: f64 = 50.0;
pub const FREQ_NOMINAL_60: f64 = 60.0;

pub const ADC_SAMPLES_50HZ_CYCLE: f64 = 156.0; /* N=fs​×Tciclo​=7812,5Hz×0,02s=156,25 */
pub const ADC_SAMPLES_60HZ_CYCLE: usize = 131;

pub const NUMBER_HARMONICS: usize = 10;

pub const MIN_AMPLITUDE_VOLTAGE: f64 = 80.0;
pub const MIN_AMPLITUDE_CURRENT: f64 = 25.0;

/// Inititial configuration
#[derive(Clone)]
pub struct MetrologyInsightConfig {
    pub avg_sec: f64,
    pub adc_samples_seconds: f64,
    pub adc_samples_per_cycle: f64,
    #[allow(dead_code)]
    pub num_harmonics: usize,
}

/// Represents a three-phase socket with current, voltage, power, and energy data.
#[derive(Default, Clone)]
pub struct MetrologyInsightSocket {
    // Voltage signals
    pub voltage_signal: MetrologyInsightSignal,

    // Current signals
    pub current_signal: MetrologyInsightSignal,

    // Phase angle metrics
    pub phase_angles: PhaseAngleMetrics,

    // Power metrics
    pub power_metrics: PowerMetrics,

    // Energy metrics
    pub energy_metrics: EnergyMetrics,
}

#[derive(Clone)]
pub struct MetrologyInsight {
    pub socket: MetrologyInsightSocket,
    pub config: MetrologyInsightConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetrologyInsightSignalType {
    Voltage,
    Current,
}

impl MetrologyInsightSignalType {
    pub fn min_amplitude(&self) -> f64 {
        match self {
            MetrologyInsightSignalType::Voltage => MIN_AMPLITUDE_VOLTAGE,
            MetrologyInsightSignalType::Current => MIN_AMPLITUDE_CURRENT,
        }
    }
}

impl Default for MetrologyInsightSignalType {
    fn default() -> Self {
        MetrologyInsightSignalType::Voltage
    }
}

/// Represents a current or voltage signal.
#[derive(Clone, Debug)]
pub struct MetrologyInsightSignal {
    pub wave: Vec<i32>,      // Signal buffer
    pub real_wave: Vec<f64>, // Real signal buffer
    pub length: usize,       // Length of the sample buffer (usually greater than 1 cycle)
    pub length_cycle: usize, // Samples in 1 cycle of the signal (less than the buffer length)
    pub calc_freq: bool,     // Indicates if the frequency should be calculated from the signal
    pub peak: f64,           // Peak value of the signal
    pub rms: f64,            // RMS value of the signal
    pub freq_nominal: f64,   // Nominal frequency (50Hz or 60Hz)
    pub freq_zc: f64,        // Frequency of the signal based on zero crossing
    #[allow(dead_code)]
    pub harmonics: [f64; NUMBER_HARMONICS], // Array of amplitudes and phases of harmonics
    #[allow(dead_code)]
    pub thd: f64, // Total harmonic distortion
    pub sc_thres: f64,       // Short circuit threshold
    pub signal_type: MetrologyInsightSignalType, // Tipo de señal (tensión o corriente)
    pub adc_factor: f64,     // ADC factor
    pub adc_scale: f64,      // ADC scale
}

impl Default for MetrologyInsightSignal {
    fn default() -> Self {
        Self {
            wave: vec![],
            real_wave: vec![],
            length: 0,
            length_cycle: 0,
            calc_freq: false,
            peak: 0.0,
            rms: 0.0,
            freq_nominal: FREQ_NOMINAL_50,
            freq_zc: 0.0,
            harmonics: [0.0; NUMBER_HARMONICS],
            thd: 0.0,
            sc_thres: 0.0,
            signal_type: MetrologyInsightSignalType::Voltage,
            adc_factor: 1.0,
            adc_scale: 1.0,
        }
    }
}

#[derive(Clone)]
pub enum PhaseDirection {
    Inductive,  // Corriente retrasa a tensión (ángulo positivo)
    Capacitive, // Corriente adelanta a tensión (ángulo negativo)
    InPhase,    // Sin desfase (casi 0°)
}

impl Default for PhaseDirection {
    fn default() -> Self {
        PhaseDirection::InPhase
    }
}

impl PhaseDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            PhaseDirection::Inductive => "Inductive (current lags voltage)",
            PhaseDirection::Capacitive => "Capacitive (current leads voltage)",
            PhaseDirection::InPhase => "In phase (no phase difference)",
        }
    }
}

#[derive(Clone, Default)]
pub struct PhaseAngleMetrics {
    pub c2v_angle: f64,            // Current-to-voltage phase difference (signed)
    pub v_angle: f64,              // Absolute voltage angle (0-360°)
    pub c_angle: f64,              // Absolute current angle (0-360°)
    pub direction: PhaseDirection, // Phase direction (inductive, capacitive, in-phase)
}

impl PhaseAngleMetrics {
    pub fn direction_description(&self) -> &'static str {
        self.direction.as_str()
    }
}

#[derive(Clone, Default)]
pub struct PowerMetrics {
    pub real_power: f64,
    pub reactive_power: f64,
    pub apparent_power: f64,
    pub power_factor: f64,
}

#[derive(Clone, Default)]
pub struct ActiveEnergyMetrics {
    pub imported: f64,
    pub exported: f64,
    pub balance: f64,
    pub q1: f64,
    pub q2: f64,
    pub q3: f64,
    pub q4: f64,
}

impl ActiveEnergyMetrics {
    /*
     * @brief Calculate the active energy by quadrant.
     * @param socket Pointer to the MetrologyInsightSocket structure.
     */
    pub fn imported(&self) -> f64 {
        self.q1 + self.q4
    }

    /*
     * @brief Calculate the active energy by quadrant.
     * @param socket Pointer to the MetrologyInsightSocket structure.
     */
    pub fn exported(&self) -> f64 {
        self.q2 + self.q3
    }

    /*
     * @brief Calculate the active energy by quadrant.
     * @param socket Pointer to the MetrologyInsightSocket structure.
     */
    pub fn balance(&self) -> f64 {
        self.imported() - self.exported()
    }
}

#[derive(Clone, Default)]
pub struct ReactiveEnergyMetrics {
    pub capacitive: f64,
    pub inductive: f64,
    pub balance: f64,
    pub q1: f64,
    pub q2: f64,
    pub q3: f64,
    pub q4: f64,
}

impl ReactiveEnergyMetrics {
    /*
     * @brief Calculate the reactive energy by quadrant.
     * @param socket Pointer to the MetrologyInsightSocket structure.
     */
    pub fn inductive(&self) -> f64 {
        self.q1 + self.q3
    }

    /*
     * @brief Calculate the reactive energy by quadrant.
     * @param socket Pointer to the MetrologyInsightSocket structure.
     */
    pub fn capacitive(&self) -> f64 {
        self.q2 + self.q4
    }

    /*
     * @brief Calculate the reactive energy by quadrant.
     * @param socket Pointer to the MetrologyInsightSocket structure.
     */
    pub fn balance(&self) -> f64 {
        (self.q1 + self.q2) - (self.q3 + self.q4)
    }
}

#[derive(Clone, Default)]
pub struct EnergyMetrics {
    pub active: ActiveEnergyMetrics,
    pub reactive: ReactiveEnergyMetrics,
}
