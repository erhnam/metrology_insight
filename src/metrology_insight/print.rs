use crate::MetrologyInsightSocket;

/*
* @brief Functions to print the data of the Metrology Insight device.
* @param data Pointer to the MetrologyInsightSocket structure.
* @note This file contains functions to print the data of the Metrology Insight device.
*/
pub fn print_voltage_signal(data: &MetrologyInsightSocket) {
    log::info!("Voltage:");
    log::info!("  Peak: {:.3} V", data.voltage_signal.peak);
    log::info!("  RMS: {:.3} V", data.voltage_signal.rms);
    log::info!("  Frequency: {:.3} Hz\n", data.voltage_signal.freq_zc);
}

/*
* @brief Print the current signal data.
* @param data Pointer to the MetrologyInsightSocket structure.
* @note This function prints the current signal data.
*/
pub fn print_current_signal(data: &MetrologyInsightSocket) {
    log::info!("Current:");
    log::info!("  Peak: {:.3} A", data.current_signal.peak);
    log::info!("  RMS: {:.3} A", data.current_signal.rms);
    log::info!("  Frequency: {:.3} Hz\n", data.current_signal.freq_zc);
}

/*
* @brief Print the power data
* @param data Pointer to the MetrologyInsightSocket structure.
* @note This function prints the power data.
*/
pub fn print_power(data: &MetrologyInsightSocket) {
    log::info!("Power:");
    log::info!("  Active: {:.3} W", data.power_metrics.real_power);
    log::info!("  Reactive: {:.3} VAR", data.power_metrics.reactive_power);
    log::info!("  Apparent: {:.3} VA", data.power_metrics.apparent_power);
    log::info!("  Factor: {:.3}\n", data.power_metrics.power_factor);
}

/*
* @brief Print the phase angle data
* @param data Pointer to the MetrologyInsightSocket structure.
* @note This function prints the phase angle data.
*/
pub fn print_phase_angle(data: &MetrologyInsightSocket) {
    log::info!("Phase Angle:");
    log::info!("  Current to Voltage Angle: {:.2}º", data.phase_angles.c2v_angle);
    log::info!("  Voltage Angle: {:.2}º", data.phase_angles.v_angle);
    log::info!("  Current Angle: {:.2}º", data.phase_angles.c_angle);
    log::info!("  Phase direction: {}\n", data.phase_angles.direction_description());
}

/*
* @brief Print the active energy data
* @param data Pointer to the MetrologyInsightSocket structure.
* @note This function prints the active energy data.
*/
pub fn print_active_energy(data: &MetrologyInsightSocket) {
    log::info!("Active Energy:");
    log::info!("  Imported Energy: {:.3} kWh", data.energy_metrics.active.imported);
    log::info!("  Exported Energy: {:.3} kWh", data.energy_metrics.active.exported);
    log::info!("  Balance: {:.3} kWh\n", data.energy_metrics.active.balance);
    log::info!("  Active Energy Q1: {:.3} kWh", data.energy_metrics.active.q1);
    log::info!("  Active Energy Q2: {:.3} kWh", data.energy_metrics.active.q2);
    log::info!("  Active Energy Q3: {:.3} kWh", data.energy_metrics.active.q3);
    log::info!("  Active Energy Q4: {:.3} kWh\n", data.energy_metrics.active.q4);
}

/*
* @brief Print the reactive energy data
* @param data Pointer to the MetrologyInsightSocket structure.
* @note This function prints the reactive energy data.
*/
pub fn print_reactive_energy(data: &MetrologyInsightSocket) {
    log::info!("Reactive Energy:");
    log::info!(
        "  Capacitive Energy: {:.3} kWh",
        data.energy_metrics.reactive.capacitive
    );
    log::info!("  Inductive Energy: {:.3} kWh", &data.energy_metrics.reactive.inductive);
    log::info!("  Balance: {:.3} kWh\n", data.energy_metrics.reactive.balance);
    log::info!("  Reactive Energy Q1: {:.3} kWh", data.energy_metrics.reactive.q1);
    log::info!("  Reactive Energy Q2: {:.3} kWh", data.energy_metrics.reactive.q2);
    log::info!("  Reactive Energy Q3: {:.3} kWh", data.energy_metrics.reactive.q3);
    log::info!("  Reactive Energy Q4: {:.3} kWh\n", data.energy_metrics.reactive.q4);
}

/*
* @brief Print all data from the Metrology Insight device.
* @param data Pointer to the MetrologyInsightSocket structure.
* @note This function prints all data from the Metrology Insight device.
*/
pub fn print_all(data: &MetrologyInsightSocket) {
    print_voltage_signal(data);
    print_current_signal(data);
    print_power(data);
    print_phase_angle(data);
    print_active_energy(data);
    print_reactive_energy(data);
}
