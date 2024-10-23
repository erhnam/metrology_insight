use super::MetrologyInsightSocket;

fn calculate_active_energy_by_cuadrant(socket: &mut MetrologyInsightSocket) 
{
	/*
	 * (3600 * 1000) is a constant used to convert joules to kilowatt-hours (kWh).
	 * 3600 is the number of seconds in an hour.
	 * Multiplying 3600 by 1000 gives the number of milliseconds in an hour, which equals 3.6 x 10^6 milliseconds.
	 * The unit of energy in joules is divided by this constant to convert the energy to kilowatt-hours.
	 * Since one kilowatt-hour is equal to 3.6 x 10^6 joules.
	 * samples_time is the time between samples.
	 */
    let samples_time: f64 = 1.0/socket.voltage_signal.freq_zc;
	let energy: f64 = socket.active_power * samples_time; // Energy in Joules
	let energy_kwh: f64 = energy / (3600.0 * 1000.0); // Energy in kWh

    if socket.active_power > 0.0 && socket.reactive_power > 0.0 {
		socket.active_energy_q1 += energy_kwh;

		return;
	}

	if socket.active_power > 0.0 && socket.reactive_power < 0.0 {
		socket.active_energy_q4 += energy_kwh;

		return;
	}

	if socket.active_power < 0.0 && socket.reactive_power > 0.0 {
		socket.active_energy_q2 += energy_kwh * (-1.0);

		return;
	}

	if socket.active_power < 0.0 && socket.reactive_power < 0.0 {
		socket.active_energy_q3 += energy_kwh * (-1.0);

		return;
	}
}

fn calculate_reactive_energy_by_cuadrant(socket: &mut MetrologyInsightSocket) 
{
	/*
	 * (3600 * 1000) is a constant used to convert joules to kilowatt-hours (kWh).
	 * 3600 is the number of seconds in an hour.
	 * Multiplying 3600 by 1000 gives the number of milliseconds in an hour, which equals 3.6 x 10^6 milliseconds.
	 * The unit of energy in joules is divided by this constant to convert the energy to kilowatt-hours.
	 * Since one kilowatt-hour is equal to 3.6 x 10^6 joules.
	 * samples_time is the time between samples.
	 */
    let samples_time: f64 = 1.0/socket.voltage_signal.freq_zc;
	let energy: f64 = socket.reactive_power * samples_time; // Energy in Joules
	let energy_kwh: f64 = energy / (3600.0 * 1000.0); // Energy in kWh

	if socket.active_power > 0.0 && socket.reactive_power > 0.0 {
		socket.reactive_energy_q1 += energy_kwh;

		return;
	}

	if socket.active_power > 0.0 && socket.reactive_power < 0.0 {
		socket.reactive_energy_q4 += energy_kwh * (-1.0);

		return;
	}

	if socket.active_power < 0.0 && socket.reactive_power > 0.0 {
		socket.reactive_energy_q2 += energy_kwh;

		return;
	}

	if socket.active_power < 0.0 && socket.reactive_power < 0.0 {
		socket.reactive_energy_q3 += energy_kwh * (-1.0);

		return;
	}
}

pub fn calculate_energy_by_cuadrant(socket: &mut MetrologyInsightSocket) {
    calculate_active_energy_by_cuadrant(socket);
    calculate_reactive_energy_by_cuadrant(socket);
}

fn calculate_imported_energy(active_energy_q1: f64, active_energy_q4: f64) -> f64 {
    active_energy_q1 + active_energy_q4
}

fn calculate_exported_energy(active_energy_q2: f64, active_energy_q3: f64) -> f64 {
    active_energy_q2 + active_energy_q3
}

fn calculate_active_balance_energy(energy_imported: f64, energy_exported: f64) -> f64 {
    energy_imported - energy_exported
}

fn calculate_inductive_energy(reactive_energy_q1: f64, reactive_energy_q3: f64) -> f64 {
    reactive_energy_q1 + reactive_energy_q3
}

fn calculate_capacitive_energy(reactive_energy_q2: f64, reactive_energy_q4: f64) -> f64 {
    reactive_energy_q2 + reactive_energy_q4
}

fn calculate_reactive_balance_energy(reactive_energy_q1: f64, reactive_energy_q2: f64, reactive_energy_q3: f64, reactive_energy_q4: f64) -> f64 {
    (reactive_energy_q1 + reactive_energy_q2) - (reactive_energy_q3 + reactive_energy_q4)
}

pub fn calculate_energy(socket: &mut MetrologyInsightSocket)
{
    /* Imported energy */
    socket.energy_imported = calculate_imported_energy(socket.active_energy_q1, socket.active_energy_q4);

    /* Exported energy */
    socket.energy_exported = calculate_exported_energy(socket.active_energy_q2, socket.active_energy_q3);

    /* Active Balance energy */
    socket.active_energy_balance = calculate_active_balance_energy(socket.energy_imported, socket.energy_exported);

    /* Inductive energy */
    socket.energy_inductive = calculate_inductive_energy(socket.reactive_energy_q1, socket.reactive_energy_q3);

    /* Capacitive energy */
    socket.energy_capacitive = calculate_capacitive_energy(socket.reactive_energy_q2, socket.reactive_energy_q4);

    /* Reactive Balance energy */
    socket.reactive_energy_balance = calculate_reactive_balance_energy(socket.reactive_energy_q1, socket.reactive_energy_q2, socket.reactive_energy_q3, socket.reactive_energy_q4);
}