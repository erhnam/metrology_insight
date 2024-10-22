mod current;
mod voltage;
mod power;
mod energy;
mod phase;
pub mod signal_processing;
pub mod generate_signal;

pub use generate_signal::generate_signals;

/// Representa un socket trifásico con datos de corriente, voltaje, potencias y energía
#[derive(Default, Clone)]
pub struct MetrologyInsightSocket {
    // Señales de voltaje
    voltage_signal: signal_processing::MetrologyInsightSignal,

    // Señales de corriente
    current_signal: signal_processing::MetrologyInsightSignal,
/*
    // Ángulo fase a fase
    c2v_angle: [f64; 3], // Diferencia de ángulo corriente a voltaje (para la misma fase)
    voltage_angle: [f64; 3], // Ángulo de voltaje respecto a la fase 0 (voltage_angle[0] siempre es cero)
    current_angle: [f64; 3], // Ángulo de corriente; I[0] es la referencia, así que current_angle[0] siempre es cero
    v_ph2ph: [f64; 3], // Voltajes fase a fase
    in_signals: f64, // I1 + I2 + I3 TRMS (debería ser igual al TRMS de I4)
    in_homopolar: f64,
    in_phases: f64,

    // Componentes simétricas
    v_phasor: [num_complex::Complex<f64>; 3], // Fasores de voltaje
    i_phasor: [num_complex::Complex<f64>; 3], // Fasores de corriente
    v_phasorsym: [num_complex::Complex<f64>; 3], // Fasores de voltaje de componentes simétricas
    i_phasorsym: [num_complex::Complex<f64>; 3], // Fasores de corriente de componentes simétricas

    // Potencias y energías
    real_power3phase: f64,       // Suma de potencias reales
    reactive_power3phase: f64,   // Suma de potencias reactivas
    apparent_power3phase: f64,   // Potencia aparente de las 3 fases
    power_factor3phase: f64,     // Factor de potencia de las 3 fases
    real_power: [f64; 3],
    reactive_power: [f64; 3],
    apparent_power: [f64; 3],
    power_factor: [f64; 3], // Factor de potencia: cos(phi)

    // Energías activas y reactivas por cuadrante
    active_energy_q1: [f64; 3],
    active_energy_q2: [f64; 3],
    active_energy_q3: [f64; 3],
    active_energy_q4: [f64; 3],
    reactive_energy_q1: [f64; 3],
    reactive_energy_q2: [f64; 3],
    reactive_energy_q3: [f64; 3],
    reactive_energy_q4: [f64; 3],

    // Energías activas y reactivas de las 3 fases por cuadrante
    active_energy_q1_3phase: f64,
    active_energy_q2_3phase: f64,
    active_energy_q3_3phase: f64,
    active_energy_q4_3phase: f64,
    reactive_energy_q1_3phase: f64,
    reactive_energy_q2_3phase: f64,
    reactive_energy_q3_3phase: f64,
    reactive_energy_q4_3phase: f64,

    // Energías importadas, exportadas y balance de energía
    energy_imported: [f64; 3],
    energy_exported: [f64; 3],
    active_energy_balance: [f64; 3],
    energy_capacitive: [f64; 3],
    energy_inductive: [f64; 3],
    reactive_energy_balance: [f64; 3],

    // Configuración de fase
    invert_phases: [bool; 3], // Array de inversión de fases
    v_min_phases: [bool; 4],  // Array de fases con mínimo voltaje.
*/
}

#[derive(Clone)]
pub struct MetrologyInsight {
    pub socket: MetrologyInsightSocket,
    pub num_phases: usize,
}

impl MetrologyInsight {
    // Inicializa los valores por defecto
    pub fn init(&mut self, voltage_signal: signal_processing::MetrologyInsightSignal, current_signal: signal_processing::MetrologyInsightSignal) -> Self {
        self.socket = MetrologyInsightSocket {
            voltage_signal: voltage_signal,
            current_signal: current_signal,
            ..Default::default() // Usar valores por defecto para otros campos
        };
        self.clone() // O simplemente `self`, si implementas `Copy`
    }

    // Procesar señales y calcular métricas
    pub fn process_signal(&mut self) {
        // Procesar las señales para la fase actual
        signal_processing::process_signal(&mut self.socket.voltage_signal, signal_processing::ADC_VOLTAGE_D2A_FACTOR);
        signal_processing::process_signal(&mut self.socket.current_signal, signal_processing::ADC_CURRENTS_D2A_FACTOR);
    }

    pub fn print_signal(&mut self) {
        println!("Voltage: ");
        println!("\tPeak: {:?}", self.socket.voltage_signal.peak);
        println!("\tFz: {:?}", self.socket.voltage_signal.freq_zc);
        println!("\tRMS: {:?}", self.socket.voltage_signal.rms);
        println!("Current: ");
        println!("\tPeak: {:?}", self.socket.current_signal.peak);
        println!("\tFz: {:?}", self.socket.current_signal.freq_zc);
        println!("\tRMS: {:?}", self.socket.current_signal.rms);
        println!("\tsc_thres: {:?}", self.socket.current_signal.sc_thres);
    }
}
