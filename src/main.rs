/*
insmod /mnt/system/ko/cv180x_saradc.ko

chmod +x metrology_insight
ln -s ./lib/ld-musl-riscv64v0p7_xthead.so.1 /lib/ld-musl-riscv64.so.1
ln -s ./usr/lib64v0p7_xthead/lp64d/libc.so /lib/libc.so

chmod 755 /lib/ld-musl-riscv64v0p7_xthead.so.1
ln -sf /lib/ld-musl-riscv64v0p7_xthead.so.1 /lib/ld-musl-riscv64.so.1

*/
#[warn(dead_code)]
use std::fs::OpenOptions;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;
use std::sync::{Arc, Mutex};
use std::thread;
use std::os::unix::io::AsRawFd;
use nix::libc::{mmap, MAP_FAILED, MAP_SHARED, PROT_READ};
use nix::{ioctl_none, ioctl_read};
use signal_hook::consts::signal::*;
use signal_hook::iterator::Signals;

mod metrology_insight;
use metrology_insight::signal_processing::MetrologyInsightSignal;

const SAMPLES_PER_CYCLE: usize = 177;
const SAMPLE_FREQUENCY: f64 = 7812.5; // 7812.5
const ADC_VOLTAGE_FACTOR: f64 = 1.65;

/* IOCTL */
const IOCTL_MAGIC: u8 = b'W';  // El mismo código mágico que en el kernel
const IOCTL_START_TIMER: u64 = 0x1;  // Define tu propio comando (si quieres usar este formato)
const IOCTL_WAIT_BUFFER_SWITCH: u64 = 0x2;
const IOCTL_REGISTER_PID: u64 = 0x3;

ioctl_none!(start_timer, IOCTL_MAGIC, IOCTL_START_TIMER);
ioctl_read!(ioctl_wait_buffer_switch, IOCTL_MAGIC, IOCTL_WAIT_BUFFER_SWITCH, i32);
ioctl_none!(ioctl_register_pid, IOCTL_MAGIC, IOCTL_REGISTER_PID);

fn main() {
	if is_module_loaded("cv180x_saradc") {
		println!("El módulo SARADC ya está cargado.");
	} else {
		load_module("cv180x_saradc");
		println!("Módulo SARADC cargado.");
	}

	// Crear un canal para señales
	let mut signals = Signals::new(&[SIGUSR1, SIGUSR2]).expect("No se pudo registrar señales");

	let (tx, rx) = mpsc::channel::<(&[i32], &[i32])>(); 
	let (tx_process_to_print, rx_process_to_print) = mpsc::channel::<()>(); 

	let config = metrology_insight::MetrologyInsightConfig {
		avg_sec: 0.02,
		adc_voltage_d2a_factor: ADC_VOLTAGE_FACTOR,
		adc_currents_d2a_factor: ADC_VOLTAGE_FACTOR,
		adc_samples_seconds: SAMPLE_FREQUENCY,
		num_harmonics: 0,
	};

	let insight = Arc::new(Mutex::new(metrology_insight::MetrologyInsight {
			socket: Default::default(),  // Default socket initialization
			config: config,
		})
	);

	thread::spawn(move || {
		//println!("¡Recibida SIGUSR1 del kernel!");
		let fd = OpenOptions::new()
		.read(true)
		.open("/dev/cvi-saradc0")
		.expect("Error opening ADC device");
		let fd = fd.as_raw_fd();
	
		unsafe {
			// Mmap para mapear el dispositivo
			let addr = mmap(
				std::ptr::null_mut(),                  // Dirección sugerida (NULL para que el kernel decida)
				708,                                    // Tamaño a mapear 2 Buffers 354 samples
				PROT_READ,                                  // Permisos (solo lectura)
				MAP_SHARED,                           // Tipo de mapeo
				fd,                                          // Descriptor de archivo
				0 as nix::libc::off_t,  			 // Offset en bytes
			);
	
			if addr == MAP_FAILED {
				eprintln!("Failed to mmap buffer");
				return;
			}
			
			/* Register PID in linux driver */
			let result = ioctl_register_pid(fd);
			if let Err(e) = result {
				eprintln!("Error al registrar el PID: {}", e);
				return;
			}
	
			// Ajustar el puntero base para que comience después del offset de 4 bytes
			let data_ptr = addr as *const i32;
	
			// Crear los slices considerando el offset
			let voltage_1 = std::slice::from_raw_parts(data_ptr, SAMPLES_PER_CYCLE);
			let current_1 = std::slice::from_raw_parts(data_ptr.add(SAMPLES_PER_CYCLE), SAMPLES_PER_CYCLE);
			let voltage_2 = std::slice::from_raw_parts(data_ptr.add(2 * SAMPLES_PER_CYCLE), SAMPLES_PER_CYCLE);
			let current_2 = std::slice::from_raw_parts(data_ptr.add(3 * SAMPLES_PER_CYCLE), SAMPLES_PER_CYCLE);
	
			// Llamamos a ioctl para iniciar el temporizador para captar waveform
			let result = start_timer(fd);
			if let Err(e) = result {
				eprintln!("Error calling ioctl to start timer");
				return;
			} else {
				println!("Timer started successfully");
			}
		
			for signal in signals.forever() {
				match signal {
					SIGUSR1 => {
						// El canal transmite los datos a través del hilo
						if tx.send((&voltage_1, &current_1)).is_err() {
							println!("Error: Receiver has dropped\n");
							break;
						}
						//println!("Buffer 1: {:?}\n", voltage_1);
					},
					SIGUSR2 => {
						// El canal transmite los datos a través del hilo
						if tx.send((&voltage_2, &current_2)).is_err() {
							println!("Error: Receiver has dropped\n");
							break;
						}
						//println!("Buffer 2: {:?}\n", voltage_2);
					},
					_ => unreachable!(),
				}
			}
		}
	});

	// Thread to process voltage/current waveform
	thread::spawn({
		let consumer_insight = Arc::clone(&insight);
		
		move || {
			loop {
				while let Ok((data_voltage_to_consume, data_current_to_consume)) = rx.recv() {
					//println!("Buffer leido {:?}\n", data_voltage_to_consume);
					//println!("Current  {:?}", data_current_to_consume);

					let voltage_signal: MetrologyInsightSignal = MetrologyInsightSignal {
						signal: moving_average(data_voltage_to_consume.to_vec(), 5),   // Buffer de la señal de voltaje
						length: SAMPLES_PER_CYCLE,              // Longitud del buffer de muestras
						integrate: false,                       // Indica si la señal debe integrarse
						calc_freq: true,                        // Indica si debe calcular la frecuencia
						..Default::default()                    // Los demás campos con valores predeterminados
					};
					
					let current_signal = MetrologyInsightSignal {
						signal: moving_average(data_current_to_consume.to_vec(), 5),   // Buffer de la señal de corriente
						length: SAMPLES_PER_CYCLE,              // Longitud del buffer de muestras
						integrate: true,                        // Indica si la señal debe integrarse
						calc_freq: false,                       // Indica si la frecuencia no debe calcularse
						..Default::default()                    // Los demás campos con valores predeterminados
					};

					let mut c_insight = consumer_insight.lock().unwrap();
					// Llamada a `process_signal` y cálculo de metrología
					c_insight.process_signal(&voltage_signal, &current_signal);
					c_insight.calculate_power_metrology();
					c_insight.calculate_energy_metrology();
					
					if tx_process_to_print.send(()).is_err() {
						eprintln!("Error: Receiver has dropped");
						break;
					}
				}
			}
		}
	});

	// Tarea del tercer hilo para ejecutar funciones cada segundo
	thread::spawn({
		let insight_print: Arc<Mutex<metrology_insight::MetrologyInsight>> = Arc::clone(&insight);
		let mut second_ctr: i32 = 0;

		move || {
			loop {
				while let Ok(()) = rx_process_to_print.recv() {
					second_ctr = (second_ctr + 1) % 50; //While measures are computed every second, always send to print, no skip.
					if second_ctr == 0 {
						let mut insight = insight_print.lock().unwrap();
						insight.print_signal();
						insight.print_power();
						insight.print_energy();
					}
				}
			}
		}
	});

	// Evitar que el hilo principal termine, haciendo una espera indefinida.
	loop {
		thread::sleep(Duration::from_secs(1));  // Espera de 60 segundos
	}

}

fn is_module_loaded(module: &str) -> bool {
	let output = Command::new("lsmod")
		.output()
		.expect("Error al ejecutar lsmod");

	let stdout = String::from_utf8_lossy(&output.stdout);
	stdout.contains(module)
}

fn load_module(module: &str) {
	let command = format!("insmod $(find / -name \"{}.ko\" 2>/dev/null)", module);
	let _ = Command::new("sh")
		.arg("-c")
		.arg(command)
		.output()
		.expect("Error al cargar el módulo");
}

#[allow(dead_code)]
fn moving_average(signal: Vec<i32>, window_size: usize) -> Vec<i32> {
	let len = signal.len();
	let mut buffer = signal.clone();
	for i in 0..len {
		let start = if i >= window_size { i - window_size } else { 0 };
		let end = i + 1;
		let sum: i32 = buffer[start..end].iter().copied().sum();
		buffer[i] = sum / (end - start) as i32; // Promedio entero
	}

	buffer
}