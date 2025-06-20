#![allow(dead_code)]

use ndarray::Array1;
use rand::Rng;
use std::f64::consts::PI;

const VPEAK: f64 = 325.0;
const IPEAK: f64 = 100.0;
const IPHASE: f64 = 0.0;
const SAMPLES_OFFSET: f64 = 0.0;
const NOISE_FREQ: f64 = 6000.0;
const NOISE_VPEAK_PERCENT: f64 = 0.0;
const NOISE_IPEAK_PERCENT: f64 = 0.0;
const NOISE_RANDOM_PERCENT: f64 = 0.0;

const FS: f64 = 7812.5; // Sampling frequency
const F: f64 = 50.0; // Frequency in Hz
const N_SAMPLES: usize = 156; // Number of samples #same number used in FW #int((Fs/F)) # Fs/F --> nearest integer
pub const VIN_TO_COUNTS: f64 = 9289.14;
pub const AMPS_TO_COUNTS: f64 = 1048.5760;

const ENABLE_HARMONICS: bool = true;
const HARM_FREQ: f64 = F * 5.0;
const VOLTAGE_HARMONICS: [(f64, f64); 2] = [
    (3.0, 0.05), // 3ª armónica al 5% de VPEAK
    (5.0, 0.02), // 5ª armónica al 2% de VPEAK
];

const CURRENT_HARMONICS: [(f64, f64); 2] = [
    (7.0, 0.06), // 3ª armónica al 5% de VPEAK
    (9.0, 0.03), // 5ª armónica al 2% de VPEAK
];

const VHPEAK: f64 = VPEAK * 0.5;
const IHPEAK: f64 = IPEAK * 0.5;

fn voltage(v: f64) -> f64 {
    v * VIN_TO_COUNTS
}

fn current(i: f64) -> f64 {
    i * AMPS_TO_COUNTS
}

fn offset(deg: f64) -> f64 {
    deg * 2.0 * PI / 360.0
}

pub fn generate_signals() -> Vec<Vec<i32>> {
    let mut rng = rand::thread_rng();
    let samples = Array1::range(0.0, N_SAMPLES as f64, 1.0);

    // Noise signals
    let noise: Vec<f64> = (0..N_SAMPLES).map(|_| rng.gen_range(0.0..1.0)).collect();

    let noise_mean: f64 = noise.iter().copied().sum::<f64>() / noise.len() as f64;

    let signal_noise_random: Vec<f64> = noise
        .iter()
        .map(|&n| {
            voltage(VPEAK) * (n - noise_mean) / noise.iter().cloned().fold(0. / 0., f64::max) * NOISE_RANDOM_PERCENT
        })
        .collect();

    let signal_noise_v: Vec<f64> = samples
        .iter()
        .map(|&s| voltage(VPEAK) * (NOISE_VPEAK_PERCENT * (offset(0.0) + 2.0 * PI * NOISE_FREQ / FS * s).sin()))
        .collect();

    let signal_noise_i: Vec<f64> = samples
        .iter()
        .map(|&s| current(IPEAK) * (NOISE_IPEAK_PERCENT * (offset(0.0) + 2.0 * PI * NOISE_FREQ / FS * s).sin()))
        .collect();

    let mut signals: Vec<Vec<i32>> = Vec::new();

    let phase_offset: f64 = 0.0;

    // Señal de voltaje para la fase
    let mut signal_v: Vec<f64> = samples
        .iter()
        .map(|&s| {
            voltage(VPEAK) * (offset(phase_offset) + 2.0 * PI * F / FS * s).sin()
                + signal_noise_v[s as usize]
                + signal_noise_random[s as usize]
        })
        .collect();

    // Señal de corriente para la fase
    let mut signal_i: Vec<f64> = samples
        .iter()
        .map(|&s| {
            current(IPEAK) * (offset(phase_offset + 90.0) + offset(IPHASE) + 2.0 * PI * F / FS * s).cos()
                + signal_noise_i[s as usize]
        })
        .collect();

    // Añadir armonías si están habilitadas
    if ENABLE_HARMONICS {
        for (harm_order, perc) in VOLTAGE_HARMONICS.iter() {
            let freq = F * harm_order;
            let vpeak = VPEAK * perc;

            signal_v.iter_mut().enumerate().for_each(|(i, s)| {
                *s += voltage(vpeak) * (offset(phase_offset) + (2.0 * PI * freq / FS * samples[i])).sin();
            });
        }

        for (harm_order, perc) in CURRENT_HARMONICS.iter() {
            let freq = F * harm_order;
            let ipeak = IPEAK * perc;

            signal_i.iter_mut().enumerate().for_each(|(i, s)| {
                *s += current(ipeak)
                    * (offset(phase_offset + 90.0) + offset(IPHASE) + (2.0 * PI * freq / FS * samples[i])).cos();
            });
        }
    }

    // Sumar el desplazamiento de muestras y truncar los valores a enteros antes de almacenarlos
    let signal_v_i32: Vec<i32> = signal_v.iter().map(|&s| (s + SAMPLES_OFFSET).trunc() as i32).collect();

    let signal_i_i32: Vec<i32> = signal_i.iter().map(|&s| (s + SAMPLES_OFFSET).trunc() as i32).collect();

    // Add offsets and truncate to integers
    signals.push(signal_v_i32);
    signals.push(signal_i_i32);

    signals
}
