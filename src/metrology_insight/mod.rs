mod current;
mod voltage_current;
mod power;
mod energy;
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

    // Ángulo fase a fase
    c2v_angle: f64, // Diferencia de ángulo corriente a voltaje (para la misma fase)
    voltage_angle: f64, // Ángulo de voltaje respecto a la fase 0 (voltage_angle[0] siempre es cero)
    current_angle: f64, // Ángulo de corriente; I[0] es la referencia, así que current_angle[0] siempre es cero

    // Power
    active_power: f64,
    reactive_power: f64,
    apparent_power: f64,
    power_factor: f64, // Factor de potencia: cos(phi)

    // Active and reactive energies by quadrant.
    active_energy_q1: f64,
    active_energy_q2: f64,
    active_energy_q3: f64,
    active_energy_q4: f64,
    reactive_energy_q1: f64,
    reactive_energy_q2: f64,
    reactive_energy_q3: f64,
    reactive_energy_q4: f64,

    // Imported energy, exported energy, and energy balance.
    energy_imported: f64,
    energy_exported: f64,
    active_energy_balance: f64,
    energy_capacitive: f64,
    energy_inductive: f64,
    reactive_energy_balance: f64,
}

#[derive(Clone)]
pub struct MetrologyInsight {
    pub socket: MetrologyInsightSocket,
    pub num_phases: usize,
}

impl MetrologyInsight {
    // Procesar señales y calcular métricas
    pub fn process_signal(&mut self, voltage_signal: signal_processing::MetrologyInsightSignal, current_signal: signal_processing::MetrologyInsightSignal) {
        self.socket = MetrologyInsightSocket {
            voltage_signal: voltage_signal,
            current_signal: current_signal,
            ..Default::default()
        };
        signal_processing::process_signal(&mut self.socket.voltage_signal, signal_processing::ADC_VOLTAGE_D2A_FACTOR);
        signal_processing::process_signal(&mut self.socket.current_signal, signal_processing::ADC_CURRENTS_D2A_FACTOR);
    }

    pub fn calculate_power_metrology(&mut self) {
        let real_power: f64 = power::calculate_real_power_from_signals(
            &self.socket.voltage_signal.signal,
            &self.socket.current_signal.signal, 
            self.socket.voltage_signal.length_cycle);

        signal_processing::average(real_power, &mut self.socket.active_power, signal_processing::AVG_SEC);

        let react_power: f64 = power::calculate_react_power_from_signals(
            &self.socket.voltage_signal.signal,
            &self.socket.current_signal.signal, 
            self.socket.voltage_signal.length_cycle);

        signal_processing::average(react_power, &mut self.socket.reactive_power, signal_processing::AVG_SEC);

        self.socket.apparent_power = power::calculate_apparent_power_from_real_and_reactive_power(
            self.socket.active_power,
            self.socket.reactive_power);

        self.socket.power_factor = power::calculate_power_factor_from_apparent_and_real_power(
            self.socket.apparent_power,
            self.socket.active_power);

        //When phase 1 voltage is not valid, use phase 1 current as reference angle. Lenght cycle and zc freq are the same on both voltage and current signals.
        let voltage_angle = voltage_current::calculate_phase_angle_from_signal_values(
            &self.socket.current_signal.signal,
            &self.socket.voltage_signal.signal,
            self.socket.voltage_signal.freq_zc,
            self.socket.voltage_signal.length_cycle);

        signal_processing::average(voltage_angle, &mut self.socket.voltage_angle, signal_processing::AVG_SEC);

        let current_angle = voltage_current::calculate_phase_angle_from_signal_values(
            &self.socket.current_signal.signal,
            &self.socket.current_signal.signal,
            self.socket.current_signal.freq_zc,
            self.socket.current_signal.length_cycle);

        signal_processing::average(current_angle, &mut self.socket.current_angle, signal_processing::AVG_SEC);

        self.socket.c2v_angle = power::calculate_phase_angle_from_power_factor_and_react_power(self.socket.power_factor, self.socket.reactive_power);

    }

    pub fn calculate_energy_metrology(&mut self) {
        energy::calculate_energy_by_cuadrant(&mut self.socket);
        energy::calculate_energy(&mut self.socket);
    }

    pub fn print_signal(&mut self) {
        println!("Voltage: ");
        println!("\tPeak: {:?}", self.socket.voltage_signal.peak);
        println!("\tFz: {:?}", self.socket.voltage_signal.freq_zc);
        println!("\tRMS: {:?}", self.socket.voltage_signal.rms);
        println!("\tAngle: {:?}", self.socket.voltage_angle);
        println!("Current: ");
        println!("\tPeak: {:?}", self.socket.current_signal.peak);
        println!("\tFz: {:?}", self.socket.current_signal.freq_zc);
        println!("\tRMS: {:?}", self.socket.current_signal.rms);
        println!("\tsc_thres: {:?}", self.socket.current_signal.sc_thres);
        println!("\tAngle: {:?}", self.socket.current_angle);
        println!("c2v Angle: {:?}\n", self.socket.c2v_angle);
    }

    pub fn print_power(&mut self) {
        println!("Power: ");
        println!("\tActive: {:?}", self.socket.active_power);
        println!("\tReactive: {:?}", self.socket.reactive_power);
        println!("\tApparent: {:?}", self.socket.apparent_power);
        println!("\tFactor: {:?}", self.socket.power_factor);
    }

    pub fn print_energy(&mut self) {
        println!("Energy: ");
        println!("\tActive:");
        println!("\t\tImported: {:?}", self.socket.energy_imported);
        println!("\t\tExported: {:?}", self.socket.energy_exported);
        println!("\t\tBalanced: {:?}", self.socket.active_energy_balance);
        println!("\t\tQ1: {:?}", self.socket.active_energy_q1);
        println!("\t\tQ2: {:?}", self.socket.active_energy_q2);
        println!("\t\tQ3: {:?}", self.socket.active_energy_q3);
        println!("\t\tQ4: {:?}", self.socket.active_energy_q4);
        println!("\tReactive:");
        println!("\t\tInductive: {:?}", self.socket.energy_inductive);
        println!("\t\tCapacitive: {:?}", self.socket.energy_capacitive);
        println!("\t\tBalanced: {:?}", self.socket.reactive_energy_balance);
        println!("\t\tQ1: {:?}", self.socket.reactive_energy_q1);
        println!("\t\tQ2: {:?}", self.socket.reactive_energy_q2);
        println!("\t\tQ3: {:?}", self.socket.reactive_energy_q3);
        println!("\t\tQ4: {:?}", self.socket.reactive_energy_q4);
    }
}
