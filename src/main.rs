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
use std::time::Instant;
use std::sync::{Arc, Mutex};
use std::thread;
use memmap2::MmapMut;
use std::os::unix::io::AsRawFd;

mod metrology_insight;
use metrology_insight::signal_processing::MetrologyInsightSignal;

const SAMPLES_PER_CYCLE: usize = 177;
const SAMPLE_FREQUENCY: f64 = 7812.5;
const TARGET_CYCLE_TIME_US: u64 = 128;
const ADC_VOLTAGE_FACTOR: f64 = 0.5; // Vmax = 3.3V / 2 Raiz 2

#[repr(C)]
struct SaradcData {
    adc_data_1: u32,
    adc_data_2: u32,
}

fn main() {
	if is_module_loaded("cv180x_saradc") {
		println!("El módulo SARADC ya está cargado.");
	} else {
		load_module("cv180x_saradc");
		println!("Módulo SARADC cargado.");
	}

	let (tx, rx) = mpsc::channel::<(Vec<i32>, Vec<i32>)>(); 

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

	/* Voltage Thread */
	thread::spawn({
		move || {
			let fd = OpenOptions::new()
				.read(true)
				.write(true)
				.open("/dev/cvi-saradc0")
				.expect("Error opening ADC device");
			let fd = fd.as_raw_fd();

			// Mmap para mapear el dispositivo
			let mut mmap = unsafe {
				MmapMut::map_mut(fd).expect("Error mapping memory")
			};
	
			// Ahora `mmap` apunta a la memoria compartida
			let shared_data: &mut SaradcData = unsafe {
				// Convertimos el puntero de memoria mapeada a nuestra estructura
				&mut *(mmap.as_mut_ptr() as *mut SaradcData)
			};

			let mut adc_voltage_values: Vec<i32> = vec![0i32; SAMPLES_PER_CYCLE];
			let mut adc_current_values: Vec<i32> = vec![0i32; SAMPLES_PER_CYCLE];

			loop {
				for i in 0..SAMPLES_PER_CYCLE {
					let cycle_start: Instant = Instant::now();

					adc_voltage_values[i] = shared_data.adc_data_1 as i32;
					adc_current_values[i] = shared_data.adc_data_2 as i32;


					while cycle_start.elapsed().subsec_micros() < TARGET_CYCLE_TIME_US as u32 {
						std::hint::spin_loop();
					}

//					delay_us(TARGET_CYCLE_TIME_US);
					//println!("Elapsed time sample: {}", cycle_start.elapsed().as_micros());
				}

				tx.send((adc_voltage_values.clone(), adc_current_values.clone())).unwrap();
			}
		}
	});

	// Tarea que lee el buffer en un bucle infinito
	thread::spawn({
		let consumer_insight = Arc::clone(&insight);
		
		move || {
			loop {
				while let Ok((data_voltage_to_consume, data_current_to_consume)) = rx.recv() {
					//moving_average(&mut data_to_consume, 5);
					//println!("Voltage  {:?}", data_voltage_to_consume);
					//println!("Current  {:?}", data_current_to_consume);

					let voltage_signal: MetrologyInsightSignal = MetrologyInsightSignal {
						signal: data_voltage_to_consume,   // Buffer de la señal de voltaje
						length: SAMPLES_PER_CYCLE,              // Longitud del buffer de muestras
						integrate: false,                       // Indica si la señal debe integrarse
						calc_freq: true,                        // Indica si debe calcular la frecuencia
						..Default::default()                    // Los demás campos con valores predeterminados
					};
					
					let current_signal = MetrologyInsightSignal {
						signal: data_current_to_consume,   // Buffer de la señal de corriente
						length: SAMPLES_PER_CYCLE,              // Longitud del buffer de muestras
						integrate: true,                        // Indica si la señal debe integrarse
						calc_freq: false,                       // Indica si la frecuencia no debe calcularse
						..Default::default()                    // Los demás campos con valores predeterminados
					};

					{
						let mut c_insight = consumer_insight.lock().unwrap();
						// Llamada a `process_signal` y cálculo de metrología
						c_insight.process_signal(&voltage_signal, &current_signal);
						c_insight.calculate_power_metrology();
						c_insight.calculate_energy_metrology();
					}
				}
			}
		}
	});

	// Tarea del tercer hilo para ejecutar funciones cada segundo
	thread::spawn({
		let insight_print: Arc<Mutex<metrology_insight::MetrologyInsight>> = Arc::clone(&insight);
		move || {
			loop {
				delay_us(1_000_000);
				let mut insight = insight_print.lock().unwrap();
				insight.print_signal();
				//insight.print_power();
				//insight.print_energy();
			}
		}
	});

	// Evitar que el hilo principal termine, haciendo una espera indefinida.
	loop {
		//thread::sleep(Duration::from_secs(1));  // Espera de 60 segundos
	}

}

fn delay_us(microseconds: u64) {
	let start = Instant::now();
	// Busy-wait loop hasta que transcurra el tiempo deseado
	while start.elapsed().as_micros() < (microseconds) as u128 {}
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
fn moving_average(signal: &mut Vec<i32>, window_size: usize) {
	let len = signal.len();
	for i in 0..len {
		let start = if i >= window_size { i - window_size } else { 0 };
		let end = i + 1;
		let sum: i32 = signal[start..end].iter().copied().sum();
		signal[i] = sum / (end - start) as i32; // Promedio entero
	}
}