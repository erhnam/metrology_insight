mod metrology_insight;

use metrology_insight::signal_processing::MetrologyInsightSignal;

fn main() {
    /* 
     * Test to generate signals, you should used your sensors
     * Voltage Signal: generated_signals[0]
     * Current Signal: generated_signals[1]
     */
    let generated_signals = metrology_insight::generate_signals();

    let voltage_signal = MetrologyInsightSignal {
        signal: generated_signals[0].clone(),    // Buffer of the voltage signal
        length: 177,          // Length of the sample buffer (usually greater than 1 cycle)
        integrate: false,     // Indicates if the signal should be integrated (e.g., for Rogowski coils)
        calc_freq: true,      // Indicates if the frequency should be calculated from the signal
        ..Default::default()  // The rest of the fields are initialized with their default values
    };
    
    let current_signal = MetrologyInsightSignal {
        signal: generated_signals[1].clone(),    // Buffer of the current signal
        length: 177,          // Length of the sample buffer (usually greater than 1 cycle)
        integrate: true,      // Indicates if the signal should be integrated (e.g., for Rogowski coils)
        calc_freq: false,     // Indicates if the frequency should be calculated from the signal
        ..Default::default()  // The rest of the fields are initialized with their default values
    };
    
    let mut insight = metrology_insight::MetrologyInsight {
        socket: Default::default(),  // Default socket initialization
    };
    
    // Call init with the configurations
    insight.process_signal(voltage_signal, current_signal);  // Process the signals
    insight.calculate_power_metrology();  // Calculate power metrology
    insight.calculate_energy_metrology();  // Calculate energy metrology
    insight.print_signal();  // Print the signal data
    insight.print_power();  // Print the power data
    insight.print_energy();  // Print the energy data
}
