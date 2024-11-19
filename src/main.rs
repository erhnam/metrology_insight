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

mod metrology_insight;
use metrology_insight::signal_processing::MetrologyInsightSignal;

const SAMPLES_PER_CYCLE: usize = 177;
const SAMPLE_FREQUENCY: f64 = 7850.0; // 7812.5
const TARGET_CYCLE_TIME_MS: u64 = 20;
const ADC_VOLTAGE_FACTOR: f64 = 1.0;

const SAMPLES_PER_BUFFER: usize = 177;
const CHANNEL_COUNT: usize = 2;

fn main() {
	if is_module_loaded("cv180x_saradc") {
		println!("El módulo SARADC ya está cargado.");
	} else {
		load_module("cv180x_saradc");
		println!("Módulo SARADC cargado.");
	}

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

	/* Thread to get voltage/current waveform */
	thread::spawn({
		move || {
			let fd = OpenOptions::new()
				.read(true)
				.open("/dev/cvi-saradc0")
				.expect("Error opening ADC device");
			let fd = fd.as_raw_fd();

			// Mmap para mapear el dispositivo
			let mmap = unsafe {
				memmap2::Mmap::map(&fd).expect("Error mapping memory")
			};
	
			let samples = unsafe {
				std::slice::from_raw_parts(mmap.as_ptr() as *const i32, CHANNEL_COUNT * SAMPLES_PER_BUFFER)
			};
			
			loop {
		
				std::thread::sleep(std::time::Duration::from_millis(TARGET_CYCLE_TIME_MS));

				// Enviar los datos a través del canal
				if tx.send((&samples[0..SAMPLES_PER_BUFFER], &samples[SAMPLES_PER_BUFFER..])).is_err() {
					eprintln!("Error: Receiver has dropped");
					break;
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
					//println!("Voltage  {:?}", data_voltage_to_consume);
					//println!("Current  {:?}", data_current_to_consume);

					let voltage_signal: MetrologyInsightSignal = MetrologyInsightSignal {
						signal: data_voltage_to_consume.to_vec(),   // Buffer de la señal de voltaje
						length: SAMPLES_PER_CYCLE,              // Longitud del buffer de muestras
						integrate: false,                       // Indica si la señal debe integrarse
						calc_freq: true,                        // Indica si debe calcular la frecuencia
						..Default::default()                    // Los demás campos con valores predeterminados
					};
					
					let current_signal = MetrologyInsightSignal {
						signal: data_current_to_consume.to_vec(),   // Buffer de la señal de corriente
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