use crate::metrology_insight::{
    energy,
    phase,
    power,
    print::print_all,
    signal, // donde esté el procesamiento de señales
    types::*,
};

impl MetrologyInsight {
    pub fn process_signal(
        &mut self,
        voltage_signal: &mut MetrologyInsightSignal,
        current_signal: &mut MetrologyInsightSignal,
    ) {
        signal::process_signal(
            &mut self.socket,
            voltage_signal,
            self.config.adc_samples_seconds,
            self.config.avg_sec,
        );

        signal::process_signal(
            &mut self.socket,
            current_signal,
            self.config.adc_samples_seconds,
            self.config.avg_sec,
        );

        phase::update_phase_angles(&mut self.socket, self.config.adc_samples_per_cycle);

        power::update_power_metrics(&mut self.socket);

        energy::update_energy_by_quadrant(&mut self.socket, self.config.adc_samples_seconds);

        energy::update_total_energy(&mut self.socket, self.config.adc_samples_seconds);
    }

    pub fn print_metrology_report(&mut self) {
        print_all(&self.socket);
    }
}
