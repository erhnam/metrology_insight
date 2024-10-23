use super::signal_processing::{ADC_VOLTAGE_D2A_FACTOR, ADC_CURRENTS_D2A_FACTOR};

pub fn calculate_real_power_from_signals(signal_v: &[i32], signal_i: &[i32], length: usize) -> f64 {
    let pfactor: f64 = ADC_VOLTAGE_D2A_FACTOR*ADC_CURRENTS_D2A_FACTOR;

    let mut power: f64 = 0.0;
    if length > 0 {
        for counter in 0..length {
            power += signal_v[counter] as f64 * signal_i[counter] as f64;
        }

        power = power / length as f64
    }

	power / pfactor
}

pub fn calculate_react_power_from_signals(v_signal: &[i32], i_signal: &[i32], length: usize) -> f64 {
    let pfactor: f64 = ADC_VOLTAGE_D2A_FACTOR*ADC_CURRENTS_D2A_FACTOR;

    let mut pwr: f64 = 0.0;
    let dephase = ((length as f64 / 4.0).round()) as usize; // 90 grados (en muestras)

    if length > 0 {
        for counter in 0..length {
            if counter >= dephase {
                pwr += v_signal[counter] as f64 * i_signal[counter - dephase] as f64;
            } else {
                pwr += v_signal[counter] as f64 * i_signal[counter + length - dephase] as f64;
            }
        }

        pwr /= length as f64;
    }

    pwr / pfactor
}

pub fn calculate_apparent_power_from_real_and_reactive_power(real_power: f64, react_power: f64) -> f64 {
    (real_power.powi(2) + react_power.powi(2)).sqrt()
}

pub fn calculate_power_factor_from_apparent_and_real_power(apparent_power: f64, real_power: f64) -> f64 {
	let mut power_factor: f64 = 0.0;

	if apparent_power != 0.0 {
		power_factor = real_power / apparent_power;
		if  power_factor > 1.0 {
			power_factor = 1.0;
		}
		if power_factor < -1.0 {
			power_factor = -1.0;
		}
	} else {
		//cannot calculate power factor
	}

	return power_factor;
}

pub fn calculate_phase_angle_from_power_factor_and_react_power(power_factor: f64, react_power: f64) -> f64 {
	let mut phase: f64 = 0.0;

	if power_factor <= 1.0 && power_factor >= -1.0 {
		phase = power_factor.acos();

		if react_power < 0.0 {
			phase = -phase;
        }
	}

	return phase;
}