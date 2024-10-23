use crate::metrology_insight::types::MetrologyInsightSocket;

/*
* @brief Functions to print the data of the Metrology Insight device.
* @param data Pointer to the MetrologyInsightSocket structure.
* @note This file contains functions to print the data of the Metrology Insight device.
*/
pub fn print_voltage_signal(data: &MetrologyInsightSocket) {
    println!("Voltage:");
    println!("  Peak: {:.3} V", data.voltage_signal.peak);
    println!("  RMS: {:.3} V", data.voltage_signal.rms);
    println!("  Frequency: {:.3} Hz\n", data.voltage_signal.freq_zc);
}

/*
* @brief Print the current signal data.
* @param data Pointer to the MetrologyInsightSocket structure.
* @note This function prints the current signal data.
*/
pub fn print_current_signal(data: &MetrologyInsightSocket) {
    println!("Current:");
    println!("  Peak: {:.3} V", data.current_signal.peak);
    println!("  RMS: {:.3} V", data.current_signal.rms);
    println!("  Frequency: {:.3} Hz\n", data.current_signal.freq_zc);
}

/*
* @brief Print the power data
* @param data Pointer to the MetrologyInsightSocket structure.
* @note This function prints the power data.
*/
pub fn print_power(data: &MetrologyInsightSocket) {
    println!("Power:");
    println!("  Active: {:.3} W", data.power_metrics.real_power);
    println!("  Reactive: {:.3} VAR", data.power_metrics.reactive_power);
    println!("  Apparent: {:.3} VA", data.power_metrics.apparent_power);
    println!("  Factor: {:.3}\n", data.power_metrics.power_factor);
}

/*
* @brief Print the phase angle data
* @param data Pointer to the MetrologyInsightSocket structure.
* @note This function prints the phase angle data.
*/
pub fn print_phase_angle(data: &MetrologyInsightSocket) {
    println!("Phase Angle:");
    println!("  Current to Voltage Angle: {:.2}º", data.phase_angles.c2v_angle);
    println!("  Voltage Angle: {:.2}º", data.phase_angles.v_angle);
    println!("  Current Angle: {:.2}º", data.phase_angles.c_angle);
    println!("  Phase direction: {}\n", data.phase_angles.direction_description());
}

/*
* @brief Print the active energy data
* @param data Pointer to the MetrologyInsightSocket structure.
* @note This function prints the active energy data.
*/
pub fn print_active_energy(data: &MetrologyInsightSocket) {
    println!("Active Energy:");
    println!("  Imported Energy: {:.3} kWh", data.energy_metrics.active.imported);
    println!("  Exported Energy: {:.3} kWh", data.energy_metrics.active.exported);
    println!("  Balance: {:.3} kWh\n", data.energy_metrics.active.balance);
    println!("  Active Energy Q1: {:.3} kWh", data.energy_metrics.active.q1);
    println!("  Active Energy Q2: {:.3} kWh", data.energy_metrics.active.q2);
    println!("  Active Energy Q3: {:.3} kWh", data.energy_metrics.active.q3);
    println!("  Active Energy Q4: {:.3} kWh\n", data.energy_metrics.active.q4);
}

/*
* @brief Print the reactive energy data
* @param data Pointer to the MetrologyInsightSocket structure.
* @note This function prints the reactive energy data.
*/
pub fn print_reactive_energy(data: &MetrologyInsightSocket) {
    println!("Reactive Energy:");
    println!(
        "  Capacitive Energy: {:.3} kWh",
        data.energy_metrics.reactive.capacitive
    );
    println!("  Inductive Energy: {:.3} kWh", &data.energy_metrics.reactive.inductive);
    println!("  Balance: {:.3} kWh\n", data.energy_metrics.reactive.balance);
    println!("  Reactive Energy Q1: {:.3} kWh", data.energy_metrics.reactive.q1);
    println!("  Reactive Energy Q2: {:.3} kWh", data.energy_metrics.reactive.q2);
    println!("  Reactive Energy Q3: {:.3} kWh", data.energy_metrics.reactive.q3);
    println!("  Reactive Energy Q4: {:.3} kWh\n", data.energy_metrics.reactive.q4);
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
