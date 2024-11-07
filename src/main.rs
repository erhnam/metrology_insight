use std::io::{Read, Seek, Write};
use std::fs::OpenOptions;
use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};
use std::thread;
use spin_sleep::sleep;

mod metrology_insight;
use metrology_insight::signal_processing::MetrologyInsightSignal;

const SAMPLES_PER_CYCLE: usize = 177;
const SAMPLE_FREQUENCY: f64 = 7812.5;
const TARGET_CYCLE_TIME_US: u128 = 120;
const ADC_VOLTAGE_FACTOR: f64 = 1.08; // Vmax = 3.3V / 2 Raiz 2

fn main() {
    if is_module_loaded("cv180x_saradc") || is_module_loaded("cv181x_saradc") {
        println!("El módulo SARADC ya está cargado.");
    } else {
        load_module("cv180x_saradc");
        load_module("cv181x_saradc");
        println!("Módulo SARADC cargado.");
    }

    let (tx, rx) = mpsc::channel::<Vec<i32>>();

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

    thread::spawn({
        move || {
            // Initialize ADC device and values vector (details omitted for brevity)
            let mut fd = OpenOptions::new()
                .read(true)
                .write(true)
                .open("/sys/class/cvi-saradc/cvi-saradc0/device/cv_saradc")
                .expect("Error opening ADC device");
        

            let adc_channel = 1;
            fd.write_all(adc_channel.to_string().as_bytes()).expect("Error al escribir en el archivo");
            
            delay_microseconds(1_000_000);

            loop {               
                let mut buffer: Vec<i32> = vec![0i32; SAMPLES_PER_CYCLE];
                let mut fd_buffer: [u8; 4] = [0; 4];
                let start_time = Instant::now(); // Hora de inicio de la tarea

                for i in 0..SAMPLES_PER_CYCLE {
                    let cycle_start: Instant = Instant::now();
                    fd_buffer.fill(0);
                    fd.seek(std::io::SeekFrom::Start(0)).expect("Seek error");
                    fd.read_exact(&mut fd_buffer).expect("ADC read error");
            
                    let read_value: std::borrow::Cow<'_, str> = String::from_utf8_lossy(&fd_buffer);        
                    buffer[i] = read_value.trim().parse::<i32>().unwrap_or(0);

                    //println!("buffer[{}] = {}", i, buffer[i]);
                    let elapsed_time_us = cycle_start.elapsed().as_micros();
                    //println!("Elapsed time sample: {}", elapsed_time_us);
                    if elapsed_time_us < TARGET_CYCLE_TIME_US {
                        delay_microseconds((TARGET_CYCLE_TIME_US - elapsed_time_us) as u64);
                    }

                    //println!("Elapsed time sample: {}", cycle_start.elapsed().as_micros());
                }

                //println!("Elapsed time cycle: {} ms", start_time.elapsed().as_millis());
                //println!("Cycle: {:?}", buffer);

                tx.send(buffer.clone()).unwrap();
            }
        }
    });

    // Tarea que lee el buffer en un bucle infinito
    thread::spawn({
        let consumer_insight = Arc::clone(&insight);
        
        move || {
            loop {
                let data_to_consume: Vec<i32> = rx.recv().unwrap();
                //moving_average(&mut data_to_consume, 5);
                //println!("Consumidor  {:?}", data_to_consume);

                let voltage_signal = MetrologyInsightSignal {
                    signal: data_to_consume,   // Buffer de la señal de voltaje
                    length: SAMPLES_PER_CYCLE,              // Longitud del buffer de muestras
                    integrate: false,                       // Indica si la señal debe integrarse
                    calc_freq: true,                        // Indica si debe calcular la frecuencia
                    ..Default::default()                    // Los demás campos con valores predeterminados
                };
                
                let current_signal = MetrologyInsightSignal {
                    signal: vec![0; SAMPLES_PER_CYCLE],   // Buffer de la señal de corriente
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
    });

    // Tarea del tercer hilo para ejecutar funciones cada segundo
    thread::spawn({
        let insight_print: Arc<Mutex<metrology_insight::MetrologyInsight>> = Arc::clone(&insight);
        move || {
            loop {
                delay_microseconds(1_000_000);
                let mut insight = insight_print.lock().unwrap();
                insight.print_signal();
                //insight.print_power();
                //insight.print_energy();
            }
        }
    });

    // Evitar que el hilo principal termine, haciendo una espera indefinida.
    loop {
        thread::sleep(Duration::from_secs(1));  // Espera de 60 segundos
    }

}

fn delay_microseconds(microseconds: u64) {
    sleep(Duration::from_micros(microseconds));
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

fn moving_average(signal: &mut Vec<i32>, window_size: usize) {
    let len = signal.len();
    for i in 0..len {
        let start = if i >= window_size { i - window_size } else { 0 };
        let end = i + 1;
        let sum: i32 = signal[start..end].iter().copied().sum();
        signal[i] = sum / (end - start) as i32; // Promedio entero
    }
}