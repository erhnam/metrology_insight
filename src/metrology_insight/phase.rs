/* ----------------- Phase Angle Functions ------------------ */

use super::types::{MetrologyInsightSocket, PhaseAngleMetrics, PhaseDirection};

#[allow(dead_code)]
/*
* @brief Calculate the phase angle from power factor and reactive power.
* @param power_factor Power factor
* @param react_power Reactive power in volt-amperes reactive (VAR)
* @return Phase angle in degrees
* @note The power factor should be between -1 and 1.
* @note The phase angle is positive for inductive loads and negative for capacitive loads.
*/
fn phase_angle_from_pf_and_react_power(power_factor: f64, react_power: f64) -> f64 {
    if power_factor < -1.0 || power_factor > 1.0 {
        return 0.0; // o panic! o un Option<f64>
    }

    let clamped_pf = power_factor.clamp(-1.0, 1.0);
    let mut phase_rad = clamped_pf.acos();

    // Si la potencia reactiva es capacitiva, el ángulo debe ser negativo
    if react_power < 0.0 {
        phase_rad = -phase_rad;
    }

    phase_rad.to_degrees()
}

#[allow(dead_code)]
/*
* @brief Calculate the phase angle from voltage and current signals.
* @param voltage Voltage signal
* @param current Current signal
* @return Phase angle in degrees
* @note The phase angle is positive for inductive loads and negative for capacitive loads.
*/
fn phase_angle_from_signals(voltage: &[f64], current: &[f64]) -> f64 {
    let dot = voltage.iter().zip(current.iter()).map(|(v, i)| v * i).sum::<f64>();
    let v_mag = voltage.iter().map(|v| v * v).sum::<f64>().sqrt();
    let i_mag = current.iter().map(|i| i * i).sum::<f64>().sqrt();

    if v_mag == 0.0 || i_mag == 0.0 {
        return 0.0; // evitar división por cero
    }

    let cos_phi = (dot / (v_mag * i_mag)).clamp(-1.0, 1.0);
    cos_phi.acos().to_degrees()
}

/*
* @brief Calculate the phase angle from voltage and current signals.
* @param voltage Voltage signal
* @param current Current signal
* @param samples_per_cycle Number of samples per cycle
* @return Phase angle in degrees
* @note The phase angle is positive for inductive loads and negative for capacitive loads.
*/
fn absolute_phase_angles_from_signals(
    voltage_signal: &[f64],
    current_signal: &[f64],
    samples_per_cycle: f64,
) -> (f64, f64) {
    fn find_first_zero_crossing(signal: &[f64]) -> Option<usize> {
        for i in 1..signal.len() {
            if (signal[i - 1] < 0.0 && signal[i] >= 0.0) || (signal[i - 1] > 0.0 && signal[i] <= 0.0) {
                return Some(i);
            }
        }
        None
    }

    fn sample_index_to_angle(sample_index: usize, samples_per_cycle: f64) -> f64 {
        let angle_deg = (sample_index as f64 / samples_per_cycle) * 360.0;
        angle_deg % 360.0
    }

    let v_index = find_first_zero_crossing(voltage_signal).unwrap_or(0);
    let c_index = find_first_zero_crossing(current_signal).unwrap_or(0);

    let v_angle = sample_index_to_angle(v_index, samples_per_cycle);
    let c_angle = sample_index_to_angle(c_index, samples_per_cycle);

    (v_angle, c_angle)
}

#[allow(dead_code)]
/*
* @brief Calculate the phase angle from apparent power, active power, and reactive power.
* @param apparent_power Apparent power in volt-amperes (VA)
* @param active_power Active power in watts
* @param react_power Reactive power in volt-amperes reactive (VAR)
* @return Phase angle in degrees
* @note The phase angle is positive for inductive loads and negative for capacitive loads.
*/
fn phase_angle_from_power_values(apparent_power: f64, active_power: f64, react_power: f64) -> f64 {
    let mut phase: f64 = 0.0;

    /* Angle can be calculated as
     * phase = acos(real_power/apparent_power) ; notes: lacks phi sign (can be solved with react_power sign)
     * phase = atan(react_power/real_power); notes: react_power might lack accuracy
     */
    if apparent_power != 0.0 {
        if active_power.abs() < apparent_power {
            phase = (active_power / apparent_power).acos();
            if react_power < 0.0 {
                phase = -phase;
            }
        } else {
            //Equal is possible, Bigger is not (if bigger condition happens it is probably due to rounding error)
            phase = 0.0;
        }
    } else {
        if active_power != 0.0 {
            phase = (react_power / active_power).atan();
        } else {
            //phase cannot be calculated
        }
    }

    phase.to_degrees()
}

/*
* @brief Calculate the phase angles from voltage and current signals.
* @param voltage_signal Voltage signal
* @param current_signal Current signal
* @param samples_per_cycle Number of samples per cycle
* @return PhaseAngleMetrics structure containing the phase angles and direction
* @note The phase angle is positive for inductive loads and negative for capacitive loads.
*/
fn all_phase_angles_from_signals(
    voltage_signal: &[f64],
    current_signal: &[f64],
    samples_per_cycle: f64,
) -> PhaseAngleMetrics {
    let (v_angle, c_angle) = absolute_phase_angles_from_signals(&voltage_signal, &current_signal, samples_per_cycle);
    let c2v_angle: f64 = phase_angle_from_signals(&voltage_signal, &current_signal);

    let direction = if c2v_angle > 1e-6 {
        PhaseDirection::Inductive
    } else if c2v_angle < -1e-6 {
        PhaseDirection::Capacitive
    } else {
        PhaseDirection::InPhase
    };

    PhaseAngleMetrics {
        c2v_angle,
        v_angle,
        c_angle,
        direction,
    }
}

/*
* @brief Update the phase angles in the MetrologyInsightSocket structure.
* @param socket Pointer to the MetrologyInsightSocket structure.
* @param voltage_signal Voltage signal
* @param current_signal Current signal
* @param samples_per_cycle Number of samples per cycle
* @note This function updates the phase angles in the MetrologyInsightSocket structure.
*/
pub fn update_phase_angles(socket: &mut MetrologyInsightSocket, samples_per_cycle: f64) {
    socket.phase_angles = all_phase_angles_from_signals(
        &socket.voltage_signal.real_wave,
        &socket.current_signal.real_wave,
        samples_per_cycle,
    );
}
