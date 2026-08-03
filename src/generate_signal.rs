//! Synthetic electrical signal generator for simulation and testing.
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

#![allow(dead_code)]

use core::f32::consts::PI;

// =========================================================================
// SIGNAL GENERATOR CONFIGURATION & CONSTANTS
// =========================================================================

/// Fundamental peak voltage amplitude (Volts). Equivalent to ~230 Vrms.
const VPEAK: f32 = 330.2; /// Peak voltage (~230 Vrms + 1.5% grid overvoltage = ~233.5 Vrms -> 330.2 Vpeak)

/// Fundamental peak current amplitude (Amperes). Equivalent to ~70.7 Arms.
const IPEAK: f32 = 63.6; /// Peak current (~45.0 Arms nominal load -> 63.6 Apeak)

/// Global current phase shift relative to voltage (in degrees). 
/// 0.0° = Unity Power Factor (PF = 1.0). Negative = Inductive, Positive = Capacitive.
const IPHASE: f32 = -18.2; /// Slightly inductive load (PF ≈ 0.95 -> acos(0.95) ≈ -18.2°)

/// Constant DC offset added to ADC sample counts (LSB).
const SAMPLES_OFFSET: f32 = 12.0; /// Small DC offset from op-amp / ADC front-end bias drift (e.g., +12 LSBs)

/// High-frequency noise component injection frequency (Hz).
const NOISE_FREQ: f32 = 6000.0; /// High-frequency switching noise (e.g., 6 kHz inverter switching frequency)

/// High-frequency voltage noise amplitude (% of VPEAK).
const NOISE_VPEAK_PERCENT: f32 = 0.0; // Put to 0.0 temporarily to validate Flicker

/// High-frequency current noise amplitude (% of IPEAK).
const NOISE_IPEAK_PERCENT: f32 = 0.005; /// 0.5% high-frequency ripple on current

/// Pseudo-random white noise amplitude (% of signal peak).
const NOISE_RANDOM_PERCENT: f32 = 0.001; /// Background thermal / quantization noise (0.1% peak-to-peak)

// =========================================================================
// SAMPLING & SYSTEM TIMING
// =========================================================================

/// Sampling frequency of the simulated ADC (Hz / Samples per second).
const FS: f32 = 8000.0;

/// Fundamental power grid frequency (Hz).
const F: f32 = 49.98; /// Real grid frequency drift (e.g., 49.98 Hz instead of exact 50.00 Hz)

/// Number of samples per buffer cycle (160 samples @ 8000 Hz = 1 cycle @ 50 Hz).
const N_SAMPLES: usize = 160;

// =========================================================================
// ADC HARDWARE SCALING FACTORS (ADS131M08 24-bit Delta-Sigma)
// =========================================================================

/// 24-bit signed ADC full-scale count limit (2^23 = 8,388,608 LSBs for 1.2 V FSR).
pub const ADC_FULL_SCALE_COUNTS: f32 = 8388608.0; 

/// Voltage input scaling factor to ADC counts (LSB/V).
/// Derived from: (ADC_FSR_COUNTS / Vref) / (Resistor_Divider_Ratio * PGA_Gain).
pub const VIN_TO_COUNTS: f32 = (ADC_FULL_SCALE_COUNTS / 1.2) / (20.55 * 11.0); // ~30915.2 LSB/V

/// Current input scaling factor to ADC counts (LSB/A).
/// Derived from: (ADC_FSR_COUNTS / Vref) / (CT_Ratio / Burden_Resistor_Ratio).
pub const AMPS_TO_COUNTS: f32 = (ADC_FULL_SCALE_COUNTS / 1.2) / (2000.0 / 100.0); // ~349525.3 LSB/A

// =========================================================================
// SIMULATION FEATURE SWITCHES
// =========================================================================

/// Enable 3-Phase signal generation (true) or fallback to single-phase (false).
pub const ENABLE_THREE_PHASE: bool = true;

/// Enable harmonic distortion overlay on voltage and current signals.
const ENABLE_HARMONICS: bool = true;

/// Realistic grid voltage harmonic distortion (EN 50160 compliant, THD-V ≈ 2.2%)
const VOLTAGE_HARMONICS: [(f32, f32); 11] = [
    (3.0,  0.015), // 3rd: 1.5% (Triplen harmonic from single-phase loads)
    (5.0,  0.012), // 5th: 1.2% (Typical 6-pulse bridge distortion)
    (7.0,  0.008), // 7th: 0.8%
    (9.0,  0.003), // 9th: 0.3%
    (11.0, 0.002), // 11th: 0.2%
    (13.0, 0.001), // 13th: 0.1%
    (15.0, 0.001), // 15th: 0.1%
    (17.0, 0.0005),// 17th: 0.05%
    (19.0, 0.0005),// 19th: 0.05%
    (21.0, 0.0002),// 21st: 0.02%
    (23.0, 0.0001),// 23rd: 0.01%
];

/// Realistic nonlinear load current distortion (VFDs, LED drivers, THD-I ≈ 8.5%)
const CURRENT_HARMONICS: [(f32, f32); 11] = [
    (3.0,  0.065), // 3rd: 6.5%
    (5.0,  0.045), // 5th: 4.5%
    (7.0,  0.025), // 7th: 2.5%
    (9.0,  0.012), // 9th: 1.2%
    (11.0, 0.008), // 11th: 0.8%
    (13.0, 0.005), // 13th: 0.5%
    (15.0, 0.003), // 15th: 0.3%
    (17.0, 0.002), // 17th: 0.2%
    (19.0, 0.001), // 19th: 0.1%
    (21.0, 0.001), // 21st: 0.1%
    (23.0, 0.0005),// 23rd: 0.05%
];

#[cfg(not(feature = "rand"))]
struct SimpleRng(u32);
#[cfg(not(feature = "rand"))]
impl SimpleRng {
    /// Returns the next pseudo-random float uniformly distributed in [0, 1).
    ///
    /// # Returns
    ///
    /// A pseudo-random `f32` in the unit interval.
    fn next_f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1103515245).wrapping_add(12345);
        (self.0 >> 8) as f32 / 16777216.0
    }
}

/// Converts a voltage in volts to ADC counts.
///
/// # Arguments
///
/// * `v` - Voltage in volts.
///
/// # Returns
///
/// The voltage scaled to ADC LSB counts.
fn voltage(v: f32) -> f32 {
    v * VIN_TO_COUNTS
}

/// Converts a current in amperes to ADC counts.
///
/// # Arguments
///
/// * `i` - Current in amperes.
///
/// # Returns
///
/// The current scaled to ADC LSB counts.
fn current(i: f32) -> f32 {
    i * AMPS_TO_COUNTS
}

/// Converts an angle in degrees to radians.
///
/// # Arguments
///
/// * `deg` - Angle in degrees.
///
/// # Returns
///
/// The equivalent angle in radians.
fn offset(deg: f32) -> f32 {
    deg * 2.0 * PI / 360.0
}

/// Computes the instantaneous phase angle in radians for a sample index.
///
/// # Arguments
///
/// * `phase_deg` - Initial phase offset in degrees.
/// * `i` - Sample index within the buffer.
///
/// # Returns
///
/// The phase angle in radians at sample `i`.
fn angle_rad(phase_deg: f32, i: usize) -> f32 {
    offset(phase_deg) + 2.0 * PI * F / FS * i as f32
}

/// Generates one buffer of samples (fundamental plus optional harmonics and noise) for a phase.
///
/// # Arguments
///
/// * `phase_deg` - Phase offset in degrees.
/// * `peak` - Peak amplitude in ADC counts.
/// * `is_voltage` - Whether to generate a voltage (true) or current (false) waveform.
/// * `noise` - Random noise samples; used only when `NOISE_RANDOM_PERCENT` is non-zero.
/// * `noise_mean` - Mean of the noise samples.
/// * `noise_max` - Maximum of the noise samples.
///
/// # Returns
///
/// A vector of `N_SAMPLES` signal values in ADC counts.
fn gen_one_signal(
    phase_deg: f32,
    peak: f32,
    is_voltage: bool,
    noise: &[f32],
    noise_mean: f32,
    noise_max: f32,
) -> alloc::vec::Vec<f32> {
    let mut sig: alloc::vec::Vec<f32> = (0..N_SAMPLES)
        .map(|i| {
            let a = angle_rad(phase_deg, i);
            if is_voltage {
                peak * a.sin()
            } else {
                peak * (a + offset(IPHASE)).sin()
            }
        })
        .collect();

    if ENABLE_HARMONICS {
        let harmonics = if is_voltage { &VOLTAGE_HARMONICS[..] } else { &CURRENT_HARMONICS[..] };
        for (harm_order, perc) in harmonics {
            let freq = F * harm_order;
            let harm_peak = peak * perc;
            for i in 0..N_SAMPLES {
                let harm_angle = offset(phase_deg) + 2.0 * PI * freq / FS * i as f32;
                if is_voltage {
                    sig[i] += harm_peak * harm_angle.sin();
                } else {
                    sig[i] += harm_peak * (harm_angle + offset(IPHASE)).sin();
                }
            }
        }
    }

    if NOISE_VPEAK_PERCENT > 0.0 && is_voltage {
        for i in 0..N_SAMPLES {
            sig[i] += peak * (NOISE_VPEAK_PERCENT * (offset(0.0) + 2.0 * PI * NOISE_FREQ / FS * i as f32).sin());
        }
    }
    if NOISE_IPEAK_PERCENT > 0.0 && !is_voltage {
        for i in 0..N_SAMPLES {
            sig[i] += peak * (NOISE_IPEAK_PERCENT * (offset(0.0) + 2.0 * PI * NOISE_FREQ / FS * i as f32).sin());
        }
    }
    if NOISE_RANDOM_PERCENT > 0.0 {
        for i in 0..N_SAMPLES {
            sig[i] += peak * (noise[i] - noise_mean) / noise_max * NOISE_RANDOM_PERCENT;
        }
    }

    sig
}

/// Generates a buffer of random noise samples (`rand` when enabled, `SimpleRng` otherwise).
///
/// # Returns
///
/// A tuple of the noise vector, its mean, and its maximum value.
fn gen_noise() -> (alloc::vec::Vec<f32>, f32, f32) {
    if NOISE_RANDOM_PERCENT > 0.0 {
        #[cfg(feature = "rand")]
        {
            let noise: alloc::vec::Vec<f32> = (0..N_SAMPLES)
                .map(|_| rand::random::<f32>())
                .collect();
            let noise_mean = noise.iter().copied().sum::<f32>() / noise.len() as f32;
            let noise_max = noise.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            (noise, noise_mean, noise_max)
        }
        #[cfg(not(feature = "rand"))]
        {
            let mut rng = SimpleRng(42);
            let noise: alloc::vec::Vec<f32> = (0..N_SAMPLES)
                .map(|_| rng.next_f32())
                .collect();
            let noise_mean = noise.iter().copied().sum::<f32>() / noise.len() as f32;
            let noise_max = noise.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            (noise, noise_mean, noise_max)
        }
    } else {
        (alloc::vec::Vec::new(), 0.0, 1.0)
    }
}

/// Converts float signal values to integer ADC samples, applying the DC offset and truncation.
///
/// # Arguments
///
/// * `v` - Float signal values.
///
/// # Returns
///
/// A vector of integer ADC counts.
fn to_i32_vec(v: alloc::vec::Vec<f32>) -> alloc::vec::Vec<i32> {
    v.into_iter().map(|s| (s + SAMPLES_OFFSET).trunc() as i32).collect()
}

/// Generates single-phase voltage and current sample buffers in ADC counts.
///
/// # Returns
///
/// A vector of two buffers: voltage first, then current.
pub fn generate_signals_monophase() -> alloc::vec::Vec<alloc::vec::Vec<i32>> {
    let (noise, noise_mean, noise_max) = gen_noise();
    let v_peak = voltage(VPEAK);
    let i_peak = current(IPEAK);
    let v = gen_one_signal(0.0, v_peak, true, &noise, noise_mean, noise_max);
    let i = gen_one_signal(0.0, i_peak, false, &noise, noise_mean, noise_max);
    alloc::vec![to_i32_vec(v), to_i32_vec(i)]
}

/// Generates three-phase (or single-phase) voltage and current sample buffers in ADC counts.
///
/// # Returns
///
/// A vector of per-channel sample buffers. In three-phase mode the order is
/// [Va, Ia, Vb, Ib, Vc, Ic, In, unused]; otherwise a two-buffer [voltage, current] set.
pub fn generate_signals() -> alloc::vec::Vec<alloc::vec::Vec<i32>> {
    if !ENABLE_THREE_PHASE {
        return generate_signals_monophase();
    }

    let (noise, noise_mean, noise_max) = gen_noise();
    let v_peak = voltage(VPEAK);
    let i_peak = current(IPEAK);

    // L1 / Phase A: Reference to 0°
    let v_a = gen_one_signal(0.0, v_peak, true, &noise, noise_mean, noise_max);
    let i_a = gen_one_signal(0.0, i_peak, false, &noise, noise_mean, noise_max);

    // L2 / Phase B: Delayed by 120° in time -> Pass +120.0 to the function
    let v_b = gen_one_signal(120.0, v_peak, true, &noise, noise_mean, noise_max);
    let i_b = gen_one_signal(120.0, i_peak, false, &noise, noise_mean, noise_max);

    // L3 / Phase C: Delayed by 240° in time -> Pass -120.0 (or 240.0)
    let v_c = gen_one_signal(-120.0, v_peak, true, &noise, noise_mean, noise_max);
    let i_c = gen_one_signal(-120.0, i_peak, false, &noise, noise_mean, noise_max);

    let unused = alloc::vec![0.0; N_SAMPLES];
    
    let i_n: alloc::vec::Vec<f32> = (0..N_SAMPLES)
        .map(|i| -(i_a[i] + i_b[i] + i_c[i]))
        .collect();

    alloc::vec![to_i32_vec(v_a), to_i32_vec(i_a),
         to_i32_vec(v_b), to_i32_vec(i_b),
         to_i32_vec(v_c), to_i32_vec(i_c),
         to_i32_vec(i_n), to_i32_vec(unused)]
}
