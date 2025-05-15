/*
insmod /mnt/system/ko/cv180x_saradc.ko

chmod +x metrology_insight
ln -s ./lib/ld-musl-riscv64v0p7_xthead.so.1 /lib/ld-musl-riscv64.so.1
ln -s ./usr/lib64v0p7_xthead/lp64d/libc.so /lib/libc.so

chmod 755 /lib/ld-musl-riscv64v0p7_xthead.so.1
ln -sf /lib/ld-musl-riscv64v0p7_xthead.so.1 /lib/ld-musl-riscv64.so.1

*/
use nix::libc::{mmap, MAP_FAILED, MAP_SHARED, PROT_READ};
use nix::{ioctl_none, ioctl_read};
use signal_hook::consts::signal::*;
use signal_hook::iterator::Signals;
#[warn(dead_code)]
use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use std::process::Command;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

mod metrology_insight;
use metrology_insight::signal_processing::ADC_SAMPLES_50HZ_CYCLE;

const SAMPLES_PER_CYCLE: usize = ADC_SAMPLES_50HZ_CYCLE;
//const ADC_VOLTAGE_FACTOR: f64 = 0.8; // 1059.91; float V_original = (adc_val * 5.4) / 4095.0;

/* IOCTL */
const IOCTL_MAGIC: u8 = b'W'; // El mismo código mágico que en el kernel
const IOCTL_START_TIMER: u64 = 0x1; // Define tu propio comando (si quieres usar este formato)
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
                std::ptr::null_mut(),  // Dirección sugerida (NULL para que el kernel decida)
                SAMPLES_PER_CYCLE * 4, // Tamaño a mapear 2 Buffers 354 samples
                PROT_READ,             // Permisos (solo lectura)
                MAP_SHARED,            // Tipo de mapeo
                fd,                    // Descriptor de archivo
                0 as nix::libc::off_t, // Offset en bytes
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
                eprintln!("Error calling ioctl to start timer: {:?}", e);
                return;
            } else {
                println!("Timer started successfully");
            }

            let mut skip_count = 0;

            for signal in signals.forever() {
                if skip_count < 156 {
                    skip_count += 1;
                    continue; // Ignorar las primeras 4 señales
                }
                match signal {
                    SIGUSR1 => {
                        // El canal transmite los datos a través del hilo
                        if tx.send((&voltage_1, &current_1)).is_err() {
                            println!("Error: Receiver has dropped\n");
                            break;
                        }
                        //println!("Buffer 1: {:?}\n", voltage_1);
                    }
                    SIGUSR2 => {
                        // El canal transmite los datos a través del hilo
                        if tx.send((&voltage_2, &current_2)).is_err() {
                            println!("Error: Receiver has dropped\n");
                            break;
                        }
                        //println!("Buffer 2: {:?}\n", voltage_2);
                    }
                    _ => unreachable!(),
                }
            }
        }
    });

    // Thread to process voltage/current waveform
    thread::spawn({
        move || {
            loop {
                while let Ok((data_voltage_to_consume, data_current_to_consume)) = rx.recv() {
                    // Constantes de calibración (antes definidas con #define)
                    const START_VALUE: f32 = 0.0;
                    const STOP_VALUE: f32 = 100000.0;
                    const STEP_VALUE: f32 = 0.1;
                    const TOLERANCE: f32 = 0.10;
                    const ADC_SCALE: f32 = 4095.0;
                    const VREF: f32 = 1.8;

                    // Voltaje objetivo (equivalente a ACTUAL_VOLTAGE)
                    const TARGET_VOLTAGE: f32 = /* pon aquí tu valor real */ 222.5;

                    // 1) Offset
                    let zero_v: f32 = {
                        let sum: u32 = data_voltage_to_consume.iter().map(|&v| v as u32).sum();
                        sum as f32 / data_voltage_to_consume.len() as f32
                    };

                    // Función de RMS parametrizada en sensibilidad
                    let calc_rms = |sensitivity: f32| {
                        let sum_sq: f32 = data_voltage_to_consume
                            .iter()
                            .map(|&raw| {
                                let v = raw as f32 - zero_v;
                                v * v
                            })
                            .sum();
                        let mean_sq = sum_sq / data_voltage_to_consume.len() as f32;
                        (mean_sq.sqrt() / ADC_SCALE) * VREF * sensitivity
                    };

                    // 2) Bucle de calibración de sensibilidad
                    let mut sensitivity = START_VALUE;
                    let mut measured = calc_rms(sensitivity);

                    while (measured - TARGET_VOLTAGE).abs() > TOLERANCE {
                        if sensitivity + STEP_VALUE <= STOP_VALUE {
                            sensitivity += STEP_VALUE;
                            measured = calc_rms(sensitivity);
                            //println!("{:.2} → {:.3}", sensitivity, measured);
                        } else {
                            eprintln!("No se pudo determinar un valor de sensibilidad válido");
                            break;
                        }
                    }

                    // 3) Una vez dentro de tolerancia, lo enviamos
                    println!(
                        "Sensibilidad calibrada: {:.2}, Voltaje RMS = {:.3} V (dentro de ±{:.1} V)",
                        sensitivity, measured, TOLERANCE
                    );
                }
            }
        }
    });
    // Evitar que el hilo principal termine, haciendo una espera indefinida.
    loop {
        thread::sleep(Duration::from_secs(1)); // Espera de 1 segundos
    }
}

fn is_module_loaded(module: &str) -> bool {
    let output = Command::new("lsmod").output().expect("Error al ejecutar lsmod");

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
