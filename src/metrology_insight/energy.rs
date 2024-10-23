use crate::metrology_insight::types::*;

const JOULES_TO_KWH: f64 = 1.0 / (3600.0 * 1000.0); // Constant to convert Joules to kWh (1 kWh = 3.6e6 J)

fn elapsed_time_seconds(socket: &MetrologyInsightSocket, adc_samples_second: f64) -> Option<f64> {
    let samples_count = socket.voltage_signal.real_wave.len() as f64;
    if samples_count == 0.0 || adc_samples_second == 0.0 {
        None
    } else {
        let sample_duration = 1.0 / adc_samples_second; // duración de 1 muestra
        Some(samples_count * sample_duration)
    }
}

/*
* @brief Calculate the active and reactive energy by quadrant.
* @param socket Pointer to the MetrologyInsightSocket structure.
* @param adc_samples_second Number of ADC samples per second.
* @note This function calculates the active and reactive energy for each quadrant
*/
fn active_energy_by_quadrant(socket: &mut MetrologyInsightSocket, adc_samples_second: f64) {
    if let Some(elapsed_time) = elapsed_time_seconds(socket, adc_samples_second) {
        let energy_joules = socket.power_metrics.real_power * elapsed_time;
        let energy_kwh = energy_joules * JOULES_TO_KWH;

        if socket.power_metrics.real_power > 0.0 {
            if socket.power_metrics.reactive_power > 0.0 {
                socket.energy_metrics.active.q1 += energy_kwh;
            } else if socket.power_metrics.reactive_power < 0.0 {
                socket.energy_metrics.active.q4 += energy_kwh;
            }
        } else if socket.power_metrics.real_power < 0.0 {
            if socket.power_metrics.reactive_power > 0.0 {
                socket.energy_metrics.active.q2 -= energy_kwh * (-1.0);
            } else if socket.power_metrics.reactive_power < 0.0 {
                socket.energy_metrics.active.q3 -= energy_kwh * (-1.0);
            }
        }
    }
}

/*
* @brief Calculate the reactive energy by quadrant.
* @param socket Pointer to the MetrologyInsightSocket structure.
* @param adc_samples_second Number of ADC samples per second.
* @note This function calculates the reactive energy for each quadrant.
*/
fn reactive_energy_by_quadrant(socket: &mut MetrologyInsightSocket, adc_samples_second: f64) {
    if let Some(elapsed_time) = elapsed_time_seconds(socket, adc_samples_second) {
        let energy_joules = socket.power_metrics.reactive_power * elapsed_time;
        let energy_kwh = energy_joules * JOULES_TO_KWH;

        if socket.power_metrics.real_power > 0.0 {
            if socket.power_metrics.reactive_power > 0.0 {
                socket.energy_metrics.reactive.q1 += energy_kwh;
            } else if socket.power_metrics.reactive_power < 0.0 {
                socket.energy_metrics.reactive.q4 -= energy_kwh * (-1.0);
            }
        } else if socket.power_metrics.real_power < 0.0 {
            if socket.power_metrics.reactive_power > 0.0 {
                socket.energy_metrics.reactive.q2 += energy_kwh;
            } else if socket.power_metrics.reactive_power < 0.0 {
                socket.energy_metrics.reactive.q3 -= energy_kwh * (-1.0);
            }
        }
    }
}

/*
* @brief Calculate the active and reactive energy by quadrant.
* @param socket Pointer to the MetrologyInsightSocket structure.
* @param adc_samples_second Number of ADC samples per second.
* @note This function calculates the active and reactive energy for each quadrant.
*/
pub fn update_energy_by_quadrant(socket: &mut MetrologyInsightSocket, adc_samples_second: f64) {
    active_energy_by_quadrant(socket, adc_samples_second);
    reactive_energy_by_quadrant(socket, adc_samples_second);
}

/*
* @brief Calculate the total energy.
* @param socket Pointer to the MetrologyInsightSocket structure.
* @note This function calculates the total energy by summing the active and reactive energies.
*/
pub fn update_total_energy(socket: &mut MetrologyInsightSocket, adc_samples_second: f64) {
    update_energy_by_quadrant(socket, adc_samples_second);

    let active = &mut socket.energy_metrics.active;
    let reactive = &mut socket.energy_metrics.reactive;

    socket.energy_metrics = EnergyMetrics {
        active: ActiveEnergyMetrics {
            imported: active.imported(),
            exported: active.exported(),
            balance: active.balance(),
            ..active.clone()
        },
        reactive: ReactiveEnergyMetrics {
            inductive: reactive.inductive(),
            capacitive: reactive.capacitive(),
            balance: reactive.balance(),
            ..reactive.clone()
        },
    }
}
