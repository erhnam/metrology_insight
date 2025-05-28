use metrology_insight::{
    generate_signals, MetrologyInsight, MetrologyInsightConfig, MetrologyInsightSignal, MetrologyInsightSignalType,
    ADC_SAMPLES_50HZ_CYCLE, AMPS_TO_COUNTS, VIN_TO_COUNTS,
};

use nix::libc::{mmap, MAP_FAILED, MAP_SHARED, PROT_READ};
use nix::{ioctl_none, ioctl_read};
use signal_hook::consts::signal::*;
use signal_hook::iterator::Signals;
use std::cmp::Reverse;
#[warn(dead_code)]
use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use std::process::Command;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use clap::Parser;

use eframe::{egui, App, Frame, NativeOptions};
use egui_plot::{Legend, Line, Plot, PlotPoints};

// Shared state holding latest buffers
struct SharedBuffers {
    volt_buffer: Vec<f64>,
    curr_buffer: Vec<f64>,
}

impl SharedBuffers {
    fn new(max_samples: usize) -> Self {
        SharedBuffers {
            volt_buffer: Vec::with_capacity(max_samples),
            curr_buffer: Vec::with_capacity(max_samples),
        }
    }

    /// Load external i32 samples, convert to f64, keep last max_samples
    fn load_samples(&mut self, volt_samples: &[f64], curr_samples: &[f64]) {
        self.volt_buffer = volt_samples.to_vec();
        self.curr_buffer = curr_samples.to_vec();
    }
}

/// GUI app that reads from shared state
struct OscilloscopeApp {
    buffers: Arc<Mutex<SharedBuffers>>,
}

impl OscilloscopeApp {
    fn new(buffers: Arc<Mutex<SharedBuffers>>) -> Self {
        Self { buffers }
    }

    fn calc_rms(buffer: &[f64]) -> f64 {
        if buffer.is_empty() {
            0.0
        } else {
            let sum_sq: f64 = buffer.iter().map(|&v| v * v).sum();
            (sum_sq / buffer.len() as f64).sqrt()
        }
    }
}

impl App for OscilloscopeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        // Lock and clone data for drawing
        let (volt_data, curr_data) = {
            let guard = self.buffers.lock().unwrap();
            (guard.volt_buffer.clone(), guard.curr_buffer.clone())
        };

        // Time step in seconds (e.g., 1s / samples per second)
        let dt = 128e-6 as f64;

        let volt_points: PlotPoints = volt_data.iter().enumerate().map(|(i, &v)| [i as f64 * dt, v]).collect();
        let curr_points: PlotPoints = curr_data.iter().enumerate().map(|(i, &v)| [i as f64 * dt, v]).collect();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Osciloscopio Voltaje y Corriente");
            Plot::new("osciloscope")
                .legend(Legend::default())
                .view_aspect(2.0)
                .show(ui, |plot_ui| {
                    //plot_ui.set_plot_bounds(PlotBounds::from_min_max([0.0, -1.5], [dt * max_samples as f64, 1.5]));
                    plot_ui.line(Line::new("Voltaje", volt_points).color(egui::Color32::RED));
                    plot_ui.line(Line::new("Corriente", curr_points).color(egui::Color32::BLUE));
                });
            ui.separator();
            ui.horizontal(|ui| {
                ui.group(|ui| {
                    ui.label("RMS Voltaje:");
                    let mut txt = format!("{:.3}", OscilloscopeApp::calc_rms(&volt_data));
                    ui.add(egui::TextEdit::singleline(&mut txt).interactive(false));
                });
                ui.add_space(20.0);
                ui.group(|ui| {
                    ui.label("RMS Corriente:");
                    let mut txt = format!("{:.3}", OscilloscopeApp::calc_rms(&curr_data));
                    ui.add(egui::TextEdit::singleline(&mut txt).interactive(false));
                });
            });
        });

        ctx.request_repaint();
    }
}

/// Metrology Insight CLI
#[derive(Parser, Debug)]
#[command(author, version, about = "Metrology Insight for Milk-V Duo", long_about = None)]
struct Args {
    /// Simulate signal samples instead of reading from hardware
    #[arg(short = 's', long = "Simulate Samples")]
    simulate: bool,
}

const VREF: f64 = 1.8; // ADC reference voltage (Milk-V Duo: 1.8V)
const ADC_RESOLUTION: f64 = 4095.0; // 12-bit ADC resolution (0 - 4095)
const ADC_INT_DIVISOR: f64 = 0.5; // Internal ADC voltage divider (3.3V → 1.65V)
const FACTOR_DIVISORS_SCALE: f64 = 1.0 / ADC_INT_DIVISOR; // Total scale factor to undo internal dividers
const ADC_VOLTAGE_SENSITIVITY: f64 = 1265.0; // ADC Voltage sensitivity (mV/V) (Milk-V Duo: 1170 mV/V)
const ADC_VOLTAGE_FACTOR: f64 = (ADC_VOLTAGE_SENSITIVITY * VREF * FACTOR_DIVISORS_SCALE) / ADC_RESOLUTION; // Final factor to convert ADC reading to original voltage

const ADC_CURRENT_SCALE: f64 = 29.03; // ADC Current sensitivity for SCT013-030 (30A/1V) (calculated from burden resistor and ratio)
const ADC_CURRENT_FACTOR: f64 = VREF / ADC_RESOLUTION; // Factor to convert ADC reading to voltage for current sensor

const SAMPLES_PER_CYCLE: usize = ADC_SAMPLES_50HZ_CYCLE as usize;
const ADC_SAMPLE_SECONDS: f64 = 7812.5; // Sampling frequency: fs × cycle time = 7812.5 Hz × 0.02s = 156.25 samples

/* IOCTL Commands */
const IOCTL_MAGIC: u8 = b'W'; // Same magic code as in kernel module
const IOCTL_START_TIMER: u64 = 0x1; // Custom command to start timer
const IOCTL_WAIT_BUFFER_SWITCH: u64 = 0x2; // Custom command to wait for buffer switch
const IOCTL_REGISTER_PID: u64 = 0x3; // Custom command to register process ID

// IOCTL commands for the kernel module
ioctl_none!(start_timer, IOCTL_MAGIC, IOCTL_START_TIMER);
ioctl_read!(ioctl_wait_buffer_switch, IOCTL_MAGIC, IOCTL_WAIT_BUFFER_SWITCH, i32);
ioctl_none!(ioctl_register_pid, IOCTL_MAGIC, IOCTL_REGISTER_PID);

fn main() -> Result<(), eframe::Error> {
    env_logger::init();

    let options = NativeOptions::default();
    let args = Args::parse();

    let max_samples = ADC_SAMPLES_50HZ_CYCLE as usize;
    let buffers = Arc::new(Mutex::new(SharedBuffers::new(max_samples)));
    let buffers_thread = buffers.clone();

    if args.simulate {
        log::info!("Simulating signals instead of reading from hardware.");
    } else {
        log::info!("Reading signals from hardware.");
        if is_module_loaded("cv180x_saradc") {
            log::info!("SARADC module is already loaded.");
        } else {
            load_module("cv180x_saradc");
            log::info!("SARADC module loaded.");
        }
    };

    // Create a channel to receive signals
    let mut signals = Signals::new(&[SIGUSR1, SIGUSR2]).expect("No se pudo registrar señales");

    let (tx, rx) = mpsc::channel::<(Vec<i32>, Vec<i32>)>();

    let (tx_process_to_print, rx_process_to_print) = mpsc::channel::<()>();

    let config = MetrologyInsightConfig {
        avg_sec: 0.02,
        adc_samples_seconds: ADC_SAMPLE_SECONDS,
        adc_samples_per_cycle: SAMPLES_PER_CYCLE as f64,
        num_harmonics: 0,
    };

    let insight = Arc::new(Mutex::new(MetrologyInsight {
        socket: Default::default(), // Default socket initialization
        config: config,
    }));

    thread::spawn(move || {
        if !args.simulate {
            let fd = OpenOptions::new()
                .read(true)
                .open("/dev/cvi-saradc0")
                .expect("Error opening ADC device");
            let fd = fd.as_raw_fd();

            unsafe {
                // Mmap to map the device
                let addr = mmap(
                    std::ptr::null_mut(),  // Suggested address (NULL lets the kernel decide)
                    SAMPLES_PER_CYCLE * 4, // Size to map: 2 buffers of 354 samples
                    PROT_READ,             // Permissions (read-only)
                    MAP_SHARED,            // Mapping type
                    fd,                    // File descriptor
                    0 as nix::libc::off_t, // Offset in bytes
                );

                if addr == MAP_FAILED {
                    log::error!("Failed to mmap buffer");
                    return;
                }

                /* Register PID in linux driver */
                let result = ioctl_register_pid(fd);
                if let Err(e) = result {
                    log::error!("Error al registrar el PID: {}", e);
                    return;
                }

                // Adjust the base pointer to start after the 4-byte offset.
                let data_ptr = addr as *const i32;

                // Create the slices taking the offset into account.
                let voltage_1 = std::slice::from_raw_parts(data_ptr, SAMPLES_PER_CYCLE);
                let current_1 = std::slice::from_raw_parts(data_ptr.add(SAMPLES_PER_CYCLE), SAMPLES_PER_CYCLE);
                let voltage_2 = std::slice::from_raw_parts(data_ptr.add(2 * SAMPLES_PER_CYCLE), SAMPLES_PER_CYCLE);
                let current_2 = std::slice::from_raw_parts(data_ptr.add(3 * SAMPLES_PER_CYCLE), SAMPLES_PER_CYCLE);

                // Call ioctl to start the timer for capturing the waveform.
                let result = start_timer(fd);
                if let Err(e) = result {
                    log::error!("Error calling ioctl to start timer: {:?}", e);
                    return;
                } else {
                    log::info!("Timer started successfully");
                }
                for signal in signals.forever() {
                    match signal {
                        SIGUSR1 => {
                            if tx.send((voltage_1.to_vec(), current_1.to_vec())).is_err() {
                                log::error!("Error: Receiver has dropped\n");
                                break;
                            }
                        }
                        SIGUSR2 => {
                            if tx.send((voltage_2.to_vec(), current_2.to_vec())).is_err() {
                                log::error!("Error: Receiver has dropped\n");
                                break;
                            }
                        }
                        _ => unreachable!(),
                    }
                }
            }
        } else {
            log::info!("Running in simulation mode, not waiting for signals.");
            // Create dummy data for simulation

            loop {
                let simulate_signals = generate_signals();

                if tx
                    .send((simulate_signals[0].clone(), simulate_signals[1].clone()))
                    .is_err()
                {
                    log::error!("Error: Receiver has dropped\n");
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }
        }
    });

    // Thread to process voltage/current waveform
    thread::spawn({
        let consumer_insight: Arc<Mutex<_>> = Arc::clone(&insight);

        move || {
            loop {
                while let Ok((data_voltage_to_consume, data_current_to_consume)) = rx.recv() {
                    /*
                                        print!(
                                            "{},",
                                            data_voltage_to_consume
                                                .iter()
                                                .map(|v| v.to_string())
                                                .collect::<Vec<_>>()
                                                .join(", ")
                                        );
                    */
                    // Determinar factores condicionalmente
                    let (voltage_adc_factor, current_adc_factor, current_adc_scale) = if args.simulate {
                        (1.0 / VIN_TO_COUNTS, 1.0 / AMPS_TO_COUNTS, 1.0)
                    } else {
                        (ADC_VOLTAGE_FACTOR, ADC_CURRENT_FACTOR, ADC_CURRENT_SCALE)
                    };

                    let mut voltage_signal = MetrologyInsightSignal {
                        wave: data_voltage_to_consume.to_vec(),
                        length: SAMPLES_PER_CYCLE,
                        calc_freq: true,
                        signal_type: MetrologyInsightSignalType::Voltage,
                        adc_factor: voltage_adc_factor,
                        ..Default::default()
                    };

                    let mut current_signal = MetrologyInsightSignal {
                        wave: data_current_to_consume.to_vec(),
                        length: SAMPLES_PER_CYCLE,
                        calc_freq: false,
                        signal_type: MetrologyInsightSignalType::Current,
                        adc_factor: current_adc_factor,
                        adc_scale: current_adc_scale,
                        ..Default::default()
                    };

                    let mut c_insight = consumer_insight.lock().unwrap();

                    // Metrology Calculation
                    c_insight.process_and_update_metrics(&mut voltage_signal, &mut current_signal);

                    if tx_process_to_print.send(()).is_err() {
                        log::error!("Error: Receiver has dropped");
                        break;
                    }
                    if tx_process_to_print.send(()).is_err() {
                        log::error!("Error: Receiver has dropped");
                        break;
                    }
                }
            }
        }
    });

    // Task of the third thread to execute functions every second
    thread::spawn({
        let insight_print: Arc<Mutex<MetrologyInsight>> = Arc::clone(&insight);
        let mut second_ctr: i32 = 0;

        move || {
            thread::sleep(Duration::from_secs(5)); // Wait 5 seconds before starting to print

            let mut reverse = false;
            loop {
                while let Ok(()) = rx_process_to_print.recv() {
                    second_ctr = (second_ctr + 1) % 50; //While measures are computed every second, always send to print, no skip.
                    if second_ctr == 0 {
                        let mut insight = insight_print.lock().unwrap();

                        insight.print_metrology_report();

                        if reverse {
                            let volt_inv: Vec<f64> =
                                insight.socket.voltage_signal.real_wave.iter().rev().cloned().collect();
                            let curr_inv: Vec<f64> =
                                insight.socket.current_signal.real_wave.iter().rev().cloned().collect();

                            buffers_thread.lock().unwrap().load_samples(&volt_inv, &curr_inv);
                            reverse = false;
                        } else {
                            buffers_thread.lock().unwrap().load_samples(
                                &insight.socket.voltage_signal.real_wave,
                                &insight.socket.current_signal.real_wave,
                            );
                            reverse = true;
                        }
                    }
                }
            }
        }
    });

    // Above threads are running, now we can wait for signals
    eframe::run_native(
        "Osciloscopio Voltaje y Corriente",
        options,
        Box::new(move |_cc| Ok(Box::new(OscilloscopeApp::new(buffers.clone())))),
    )
}

/*
* @brief Check if the module is loaded
* @param module Name of the module to check
* @return true if the module is loaded, false otherwise
*/
fn is_module_loaded(module: &str) -> bool {
    let output = Command::new("lsmod").output().expect("Error al ejecutar lsmod");

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.contains(module)
}

/*
* @brief Load a kernel module
* @param module Name of the module to load
*/
fn load_module(module: &str) {
    let command = format!("insmod $(find / -name \"{}.ko\" 2>/dev/null)", module);
    let _ = Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()
        .expect("Error al cargar el módulo");
}
