mod metrology_insight;

use metrology_insight::signal_processing::MetrologyInsightSignal;

fn main() {
    let num_phases = 1; // Especificar el número de fases (1, 2 o 3)

    let generated_signals = metrology_insight::generate_signals(num_phases);

    //println!("V: {:?}\n", generated_signals[0]);
    //println!("I: {:?}\n", generated_signals[1]);
    let voltage_signal = MetrologyInsightSignal {
        signal: generated_signals[(num_phases - 1) * 2].clone(),    // Buffer de la señal
        length: 177,          // Longitud del buffer de muestras (usualmente mayor a 1 ciclo)
        integrate: false,     // Indica si la señal debe ser integrada (e.g., para bobinas de Rogowski)
        calc_freq: true,      // Indica si la frecuencia debe ser calculada desde la señal
        ..Default::default()  // El resto de los campos se inicializan con sus valores por defecto
    };

    let current_signal = MetrologyInsightSignal {
        signal: generated_signals[(num_phases - 1) * 2 + 1].clone(),    // Buffer de la señal
        length: 177,          // Longitud del buffer de muestras (usualmente mayor a 1 ciclo)
        integrate: true,      // Indica si la señal debe ser integrada (e.g., para bobinas de Rogowski)
        calc_freq: false,     // Indica si la frecuencia debe ser calculada desde la señal
        ..Default::default()  // El resto de los campos se inicializan con sus valores por defecto
    };

    let mut insight = metrology_insight::MetrologyInsight {
        socket: Default::default(),
        num_phases: num_phases,
    };

    // Llama a init con las configuraciones
    insight.process_signal(voltage_signal, current_signal);
    insight.calculate_power_metrology();
    insight.print_signal();
    insight.print_power();
}
