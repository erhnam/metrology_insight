use crate::{MetrologyInsightSignal, MetrologyInsightSocket, PowerMetrics};

#[allow(dead_code)]

/* ----------------- Real Power Functions ------------------ */

/*
* @brief Calculate the real power from RMS voltage, RMS current, and power factor.
* @param voltage_rms RMS voltage
* @param current_rms RMS current
* @param power_factor Power factor
* @return Real power in watts
* @note The power factor should be between -1 and 1.
 */
fn real_power_from_rms_and_power_factor(voltage_rms: f64, current_rms: f64, power_factor: f64) -> f64 {
    voltage_rms * current_rms * power_factor
}

/*
* @brief Calculate the real power from RMS voltage and RMS current.
* @param voltage_rms RMS voltage
* @param current_rms RMS current
* @return Real power in watts
* @note This function assumes a power factor of 1.0 (purely resistive load).
*/
fn real_power_from_signals(signal_v: &[f64], signal_i: &[f64]) -> f64 {
    if signal_v.is_empty() || signal_v.len() != signal_i.len() {
        return 0.0; // o mejor: Option<f64> para manejar error
    }
    signal_v.iter().zip(signal_i.iter()).map(|(&v, &i)| v * i).sum::<f64>() / signal_v.len() as f64
}

/* ----------------- React Power Functions ------------------ */

/*
* @brief Calculate the apparent power from RMS voltage and RMS current.
* @param voltage_rms RMS voltage
* @param current_rms RMS current
* @return Apparent power in volt-amperes (VA)
* @note This function assumes a power factor of 1.0 (purely resistive load).
*/
fn reactive_power_from_apparent_and_active(apparent_power: f64, active_power: f64) -> f64 {
    let mut react_power: f64 = 0.0;

    if active_power < apparent_power {
        react_power = (apparent_power.powi(2) - active_power.powi(2)).sqrt();
    }

    react_power
}

/* ----------------- Apparent Power Functions ------------------ */

/*
* @brief Calculate the apparent power from RMS voltage and RMS current.
* @param voltage_rms RMS voltage
* @param current_rms RMS current
* @return Apparent power in volt-amperes (VA)
* @note This function assumes a power factor of 1.0 (purely resistive load).
*/
fn apparent_power_from_rms(voltage_rms: f64, current_rms: f64) -> f64 {
    voltage_rms * current_rms
}

/* ----------------- Power Factor Functions ------------------ */

/*
* @brief Calculate the apparent power from real and reactive power.
* @param real_power Real power in watts
* @param react_power Reactive power in volt-amperes reactive (VAR)
* @return Apparent power in volt-amperes (VA)
* @note This function assumes a power factor of 1.0 (purely resistive load).
*/
fn power_factor_from_apparent_and_real(apparent_power: f64, real_power: f64) -> f64 {
    if apparent_power.abs() > 0.0 {
        (real_power / apparent_power).clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

/*
* @brief Calculate the power factor from real and reactive power.
* @param real_power Real power in watts
* @param react_power Reactive power in volt-amperes reactive (VAR)
* @return Power factor (dimensionless)
*/
fn calculate_all_power_metrics(
    voltage_signal: &mut MetrologyInsightSignal,
    current_signal: &mut MetrologyInsightSignal,
) -> PowerMetrics {
    // Real power a partir de RMS y factor de potencia
    let real_power = real_power_from_signals(&voltage_signal.real_wave, &current_signal.real_wave);

    // Potencia aparente a partir de RMS
    let apparent_power = apparent_power_from_rms(voltage_signal.rms, current_signal.rms);

    // Potencia reactiva a partir de aparente y real
    let reactive_power = reactive_power_from_apparent_and_active(apparent_power, real_power);

    // Factor de potencia recalculado para asegurar coherencia
    let power_factor_calc = power_factor_from_apparent_and_real(apparent_power, real_power);

    PowerMetrics {
        real_power,
        reactive_power,
        apparent_power,
        power_factor: power_factor_calc,
    }
}

pub fn update_power_metrics(socket: &mut MetrologyInsightSocket) {
    socket.power_metrics = calculate_all_power_metrics(&mut socket.voltage_signal, &mut socket.current_signal);
}
