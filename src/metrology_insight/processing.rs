use crate::{
    print_all, process_signal, update_phase_angles, update_power_metrics, update_total_energy, MetrologyInsight,
    MetrologyInsightSignal,
};

impl MetrologyInsight {
    /*
     * @brief Process the voltage and current signals.
     * @param voltage_signal Pointer to the voltage signal.
     * @param current_signal Pointer to the current signal.
     */
    pub fn process_and_update_metrics(
        &mut self,
        voltage_signal: &mut MetrologyInsightSignal,
        current_signal: &mut MetrologyInsightSignal,
    ) {
        process_signal(
            &mut self.socket,
            voltage_signal,
            self.config.adc_samples_seconds,
            self.config.avg_sec,
        );

        process_signal(
            &mut self.socket,
            current_signal,
            self.config.adc_samples_seconds,
            self.config.avg_sec,
        );

        update_phase_angles(&mut self.socket, self.config.adc_samples_per_cycle);

        update_power_metrics(&mut self.socket);

        update_total_energy(&mut self.socket, self.config.adc_samples_seconds);
    }

    /*
     * @brief Process the voltage signal.
     * @param voltage_signal Pointer to the voltage signal.
     */
    pub fn print_metrology_report(&mut self) {
        print_all(&self.socket);
    }
}
