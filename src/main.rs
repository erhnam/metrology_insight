use std::io::{self, Read, Seek, Write};
use std::fs::OpenOptions;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::sync::mpsc;
use std::time::{Duration, Instant};

mod metrology_insight;
use metrology_insight::signal_processing::MetrologyInsightSignal;

const SAMPLES_PER_CYCLE: usize = 177;
const SAMPLE_FREQUENCY: f64 = 7812.5;
const TARGET_CYCLE_TIME_US: u128 = 128;
const VOLTAGE_REF: f64 = 3.3;
const ADC_BIT_RESOLUTION: u32 = 4095;
const ADC_OFFSET: f64 = 540.0;
const ADC_VOLTAGE_FACTOR: f64 = 1.16; // Vmax = 3.3V / 2 Raiz 2
const ADC_THRESHOLD: i32 = 10;

fn main() {
    if is_module_loaded("cv180x_saradc") || is_module_loaded("cv181x_saradc") {
        println!("El módulo SARADC ya está cargado.");
    } else {
        load_module("cv180x_saradc");
        load_module("cv181x_saradc");
        println!("Módulo SARADC cargado.");
    }
    // Initialize ADC device and values vector (details omitted for brevity)
    let fd = Arc::new(Mutex::new(
        OpenOptions::new()
            .read(true)
            .write(true)
            .open("/sys/class/cvi-saradc/cvi-saradc0/device/cv_saradc")
            .expect("Error opening ADC device"),
    ));

    let mut adc_min_value: i32 = i32::MIN;
    let mut adc_max_value: i32 = i32::MAX;

    let adc_channel = 2;
    fd.lock().unwrap().write_all(adc_channel.to_string().as_bytes()).expect("Error al escribir en el archivo");

    let adc_values: Arc<Mutex<Vec<i32>>> = Arc::new(Mutex::new(Vec::with_capacity(SAMPLES_PER_CYCLE)));

    let adc_values_clone: Arc<Mutex<Vec<i32>>> = Arc::clone(&adc_values);

    calibrate_sensor(fd.clone(), &mut adc_min_value,  &mut adc_max_value);

    let config = metrology_insight::MetrologyInsightConfig {
        avg_sec: 0.02,
        adc_voltage_d2a_factor: ADC_VOLTAGE_FACTOR,
        adc_currents_d2a_factor: ADC_VOLTAGE_FACTOR,
        adc_samples_seconds: SAMPLE_FREQUENCY,
        num_harmonics: 0,
    };

    let mut insight = metrology_insight::MetrologyInsight {
        socket: Default::default(),  // Default socket initialization
        config: config,
    };

    // Get a second average
    for _ in 0..50 {
        // Channel for notifying the capture_samples thread
        let (tx, rx): (mpsc::Sender<()>, mpsc::Receiver<()>) = mpsc::channel();
        let fd_clone: Arc<Mutex<std::fs::File>> = Arc::clone(&fd);
        adc_values.lock().unwrap().clear();
/*
        // Hilo para detectar el cruce por cero y notificar al hilo de captura
        thread::spawn({
            let tx = tx.clone();
            let fd_clone = Arc::clone(&fd_clone);
            move || {
                detect_zero_crossing(fd_clone, tx, &adc_min_value, &adc_max_value, ADC_THRESHOLD);
            }
        });

        // Hilo de captura espera la señal de cruce por cero y luego empieza a capturar muestras
        let capture_handle = thread::spawn({
            let fd_clone = Arc::clone(&fd_clone);
            let adc_values_clone = Arc::clone(&adc_values);
            move || {
                capture_samples(fd_clone, rx, adc_values_clone);
            }
        });

        // Esperar a que el hilo de captura termine antes de procesar
        capture_handle.join().expect("Failed to join capture thread");
*/          
        capture_samples(fd_clone, rx, Arc::clone(&adc_values));
        moving_average(&mut adc_values_clone.lock().unwrap().clone(), 5);
        //println!("{:?}", adc_values_clone.lock().unwrap().clone());
        let voltage_signal = MetrologyInsightSignal {
            signal: adc_values_clone.lock().unwrap().clone(),   // Buffer de la señal de voltaje
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

        // Llamada a `process_signal` y cálculo de metrología
        insight.process_signal(&voltage_signal, &current_signal);
        insight.calculate_power_metrology();
        insight.calculate_energy_metrology();
    }

    insight.print_signal();
    insight.print_power();
    insight.print_energy();
}

fn delay_microseconds(microseconds: u128) {
    let start = Instant::now();
    while start.elapsed().as_micros() < microseconds as u128 {}
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

// Define the function that performs zero-crossing detection
fn detect_zero_crossing(fd: Arc<Mutex<std::fs::File>>, tx: mpsc::Sender<()>, adc_min_value: &i32, adc_max_value: &i32, adc_threshold: i32) {
    let mut buffer: [u8; 4] = [0; 4];
    let median = (adc_max_value + adc_min_value) / 2;

    loop {
        let mut fd_locked = fd.lock().unwrap();
        fd_locked.seek(std::io::SeekFrom::Start(0)).expect("Seek error");
        fd_locked.read_exact(&mut buffer).expect("ADC read error");

        let read_value: std::borrow::Cow<'_, str> = String::from_utf8_lossy(&buffer);
        let adc_value = read_value.trim().parse::<i32>().unwrap_or(0);

        // Verificar que el adc_value esté dentro del rango deseado y sea mayor que el anterior
        if adc_value < median && adc_value > median - adc_threshold  {
            tx.send(()).expect("Zero-crossing notification send error");
            break;
        }
    }
}

// Function for capturing samples in one cycle
fn capture_samples(fd: Arc<Mutex<std::fs::File>>, rx: mpsc::Receiver<()>, adc_values: Arc<Mutex<Vec<i32>>>) {
    let mut buffer: [u8; 4] = [0; 4];
    //rx.recv().expect("Failed to receive zero-crossing signal in capture thread");
    //let start_time: Instant = Instant::now();

    let mut fd_locked = fd.lock().unwrap();

    while adc_values.lock().unwrap().len() < SAMPLES_PER_CYCLE {
        buffer.fill(0);
        let cycle_start = Instant::now();
        fd_locked.seek(std::io::SeekFrom::Start(0)).expect("Seek error");
        fd_locked.read_exact(&mut buffer).expect("ADC read error");

        let read_value: std::borrow::Cow<'_, str> = String::from_utf8_lossy(&buffer);

        let adc_value: i32 = read_value.trim().parse::<i32>().unwrap_or(0);

        //adc_values.lock().unwrap().push(map_value_to_centered_range(adc_value));
        adc_values.lock().unwrap().push(adc_value);

        let elapsed_time_us = cycle_start.elapsed().as_micros();
        if elapsed_time_us < TARGET_CYCLE_TIME_US {
            delay_microseconds(TARGET_CYCLE_TIME_US - elapsed_time_us);
        }

    }
    // Imprimir el tiempo transcurrido en milisegundos
    //println!("Captura completada en {} ms",  (start_time.elapsed().as_micros() / 1000) as f64);
}

fn calibrate_sensor(fd: Arc<Mutex<std::fs::File>>, min_value: &mut i32, max_value: &mut i32) {
    *min_value = i32::MAX; // Inicializa el mínimo al valor máximo posible
    *max_value = i32::MIN; // Inicializa el máximo al valor mínimo posible

    let mut buffer: [u8; 4] = [0; 4];

    for _ in 0..16384 {
        let mut fd_locked = fd.lock().unwrap();
        buffer.fill(0);
        fd_locked.seek(std::io::SeekFrom::Start(0)).expect("Seek error");
        fd_locked.read_exact(&mut buffer).expect("ADC read error");

        let cycle_start: Instant = Instant::now();

        let read_value: std::borrow::Cow<'_, str> = String::from_utf8_lossy(&buffer);
        let adc_value: i32 = read_value.trim().parse::<i32>().unwrap_or(0);

        // Actualiza el mínimo y el máximo
        if adc_value < *min_value {
            *min_value = adc_value;
        }
        if adc_value > *max_value {
            *max_value = adc_value;
        }

        let elapsed_time_us = cycle_start.elapsed().as_micros();
        if elapsed_time_us < TARGET_CYCLE_TIME_US {
            delay_microseconds(TARGET_CYCLE_TIME_US - elapsed_time_us);
        }
    }
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