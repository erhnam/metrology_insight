//! Flickermeter implementation constants and filters (IEC 61000-4-15).
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

// Minimum squared voltage sample to seed avg_rms (avoids 2-min initial transient)
pub const FLICKER_SEED_THRESHOLD_SQ: f32 = 10.0;
// Long-term RMS time constant for avg_rms IIR filter (seconds, ~1 minute)
pub const FLICKER_RMS_TC_SECONDS: f32 = 60.0;
// Minimum RMS guard to avoid division-by-zero in normalization
pub const FLICKER_MIN_RMS_GUARD: f32 = 1.0;
// Block 3 high-pass filter cutoff frequency (Hz) per IEC 61000-4-15
pub const FLICKER_HPF_CUTOFF_HZ: f32 = 0.05;
// Block 4 smoothing time constant (seconds = 300 ms) per IEC 61000-4-15
pub const FLICKER_SMOOTH_TC_SECONDS: f32 = 0.3;
// Minimum samples needed in PstClassifier before computing Pst
pub const FLICKER_PST_MIN_SAMPLES: u32 = 100;

pub const SOS_BW_35HZ: [[f32; 6]; 3] = [
    [6.3954004187e-12, 1.2790800837e-11, 6.3954004187e-12, 1.0000000000e+00, -1.9475393226e+00, 9.4827537525e-01],
    [1.0000000000e+00, 2.0000000000e+00, 1.0000000000e+00, 1.0000000000e+00, -1.9611295301e+00, 9.6187071893e-01],
    [1.0000000000e+00, 2.0000000000e+00, 1.0000000000e+00, 1.0000000000e+00, -1.9851227113e+00, 9.8587296816e-01],
];

pub const SOS_WEIGHTING: [[f32; 6]; 2] = [
    [2.8721268961e-05, 5.7442537921e-05, 2.8721268961e-05, 1.0000000000e+00, -1.9935916833e+00, 9.9364321833e-01],
    [1.0000000000e+00, -1.9982110587e+00, 9.9821105871e-01, 1.0000000000e+00, -1.9819845161e+00, 9.8200092041e-01],
];

/// A chain of cascaded biquad second-order sections (Direct Form II Transposed).
#[derive(Debug, Clone)]
pub struct BiquadChain<const N: usize> {
    sos: [[f32; 6]; N],
    z1: [f32; N],
    z2: [f32; N],
}

impl<const N: usize> BiquadChain<N> {
    /// Creates a new biquad chain with the given second-order sections.
    ///
    /// # Arguments
    ///
    /// * `sos` - Array of second-order sections, each with 6 coefficients [b0, b1, b2, a0, a1, a2].
    ///
    /// # Returns
    ///
    /// A `BiquadChain` with all state variables initialized to zero.
    pub fn new(sos: [[f32; 6]; N]) -> Self {
        Self {
            sos,
            z1: [0.0; N],
            z2: [0.0; N],
        }
    }

    /// Processes a single sample through all cascaded biquad sections.
    ///
    /// # Arguments
    ///
    /// * `input` - Input sample to filter.
    ///
    /// # Returns
    ///
    /// The filtered output sample after the final section.
    pub fn process(&mut self, input: f32) -> f32 {
        let mut x = input;
        for i in 0..N {
            let b0 = self.sos[i][0];
            let b1 = self.sos[i][1];
            let b2 = self.sos[i][2];
            let a1 = self.sos[i][4];
            let a2 = self.sos[i][5];

            // Direct Form II Transposed
            let y = b0 * x + self.z1[i];
            self.z1[i] = b1 * x - a1 * y + self.z2[i];
            self.z2[i] = b2 * x - a2 * y;
            x = y;
        }
        x
    }
}

/// IEC 61000-4-15 flicker meter implementing Blocks 1-4 plus a Pst classifier.
#[derive(Debug, Clone)]
pub struct FlickerMeter {
    // Block 1 & 2
    avg_rms: f32,
    initialized: bool,
    
    // Block 3: HPF 0.05Hz
    b3_hp_prev_in: f32,
    b3_hp_prev_out: f32,
    
    // Block 3: IIR cascaded SOS filters
    bw_filter: BiquadChain<3>,
    wt_filter: BiquadChain<2>,
    
    // Block 4: Squaring and smoothing (300ms time constant)
    b4_smooth_prev: f32,
    
    pub p_inst: f32,
    pub pst_classifier: PstClassifier,
}

impl Default for FlickerMeter {
    /// Creates a default `FlickerMeter` via `new()`.
    fn default() -> Self {
        Self::new()
    }
}

impl FlickerMeter {
    /// Creates a new `FlickerMeter` with a 230 V initial RMS reference and reset filter state.
    ///
    /// # Returns
    ///
    /// A `FlickerMeter` ready to accept samples.
    pub fn new() -> Self {
        Self {
            avg_rms: 230.0,
            initialized: false,
            b3_hp_prev_in: 0.0,
            b3_hp_prev_out: 0.0,
            bw_filter: BiquadChain::new(SOS_BW_35HZ),
            wt_filter: BiquadChain::new(SOS_WEIGHTING),
            b4_smooth_prev: 0.0,
            p_inst: 0.0,
            pst_classifier: PstClassifier::default(),
        }
    }

    /// Updates the initial `avg_rms` reference to the configured nominal voltage.
    ///
    /// Must be called from `apply_config()` when the nominal voltage changes at runtime.
    ///
    /// # Arguments
    ///
    /// * `nominal_v` - Nominal RMS voltage used to pre-seed `avg_rms`.
    pub fn set_nominal_voltage(&mut self, nominal_v: f32) {
        if !self.initialized {
            // Pre-seed avg_rms as nominal_v² (peak² / 2 = Vrms²)
            self.avg_rms = nominal_v * nominal_v;
        }
    }

    /// Processes a single voltage sample through the IEC 61000-4-15 flicker chain.
    ///
    /// # Arguments
    ///
    /// * `v_in` - Instantaneous voltage sample in volts.
    /// * `fs` - Sampling frequency in Hz (typically 8000).
    pub fn process_sample(&mut self, v_in: f32, fs: f32) {
        let v_sq = v_in * v_in;
        if !self.initialized && v_sq > FLICKER_SEED_THRESHOLD_SQ {
            // Seed with peak squared / 2 to avoid 2-minute initial transient
            self.avg_rms = v_sq * 0.5;
            self.initialized = true;
        }

        // Update long-term RMS (approx 1 minute time constant, IEC 61000-4-15 Block 1)
        let alpha_rms = 1.0 / (fs * FLICKER_RMS_TC_SECONDS);
        self.avg_rms = self.avg_rms * (1.0 - alpha_rms) + v_sq * alpha_rms;
        
        let v_rms = self.avg_rms.sqrt().max(FLICKER_MIN_RMS_GUARD);
        
        // Block 1: Normalization
        let v_pu = v_in / (v_rms * core::f32::consts::SQRT_2);
        
        // Block 2: Demodulation
        let v_demod = v_pu * v_pu;
        
        // Block 3: High Pass (FLICKER_HPF_CUTOFF_HZ) per IEC 61000-4-15
        let rc_hp = 1.0 / (2.0 * core::f32::consts::PI * FLICKER_HPF_CUTOFF_HZ);
        let alpha_hp = rc_hp / (rc_hp + 1.0 / fs);
        let b3_hp_out = alpha_hp * (self.b3_hp_prev_out + v_demod - self.b3_hp_prev_in);
        self.b3_hp_prev_in = v_demod;
        self.b3_hp_prev_out = b3_hp_out;
        
        // Block 3: Butterworth 6th Order LPF (35Hz) + Weighting Filter
        let bw_out = self.bw_filter.process(b3_hp_out);
        let wt_out = self.wt_filter.process(bw_out);
        
        // Block 4: Squaring and Smoothing (FLICKER_SMOOTH_TC_SECONDS) per IEC 61000-4-15
        let block4_in = wt_out * wt_out;
        let alpha_smooth = (1.0 / fs) / (FLICKER_SMOOTH_TC_SECONDS + 1.0 / fs);
        let b4_out = self.b4_smooth_prev + alpha_smooth * (block4_in - self.b4_smooth_prev);
        self.b4_smooth_prev = b4_out;
        
        // True P_inst unit
        self.p_inst = b4_out;
        self.pst_classifier.add_sample(b4_out);
    }

    /// Calculates the short-term flicker severity Pst from the classifier histogram.
    ///
    /// # Returns
    ///
    /// The Pst value, or 0.0 if fewer than `FLICKER_PST_MIN_SAMPLES` samples were collected.
    pub fn calculate_pst(&self) -> f32 {
        self.pst_classifier.calculate_pst()
    }

    /// Resets the internal Pst classifier histogram and sample counter.
    pub fn reset_pst(&mut self) {
        self.pst_classifier.reset();
    }
}

pub const FLICKER_BINS: usize = 64;
pub const FLICKER_MIN_P: f32 = 0.001;
pub const FLICKER_MAX_P: f32 = 100.0;

/// Logarithmic-binned classifier for P_inst values used to compute Pst percentiles.
#[derive(Debug, Clone, Copy)]
pub struct PstClassifier {
    pub histogram: [u32; FLICKER_BINS],
    pub total_samples: u32,
}

impl Default for PstClassifier {
    /// Creates a `PstClassifier` with an all-zero histogram and no recorded samples.
    ///
    /// # Returns
    ///
    /// An empty `PstClassifier`.
    fn default() -> Self {
        Self {
            histogram: [0; FLICKER_BINS],
            total_samples: 0,
        }
    }
}

impl PstClassifier {
    /// Clears the histogram and resets the total sample counter.
    pub fn reset(&mut self) {
        self.histogram = [0; FLICKER_BINS];
        self.total_samples = 0;
    }

    /// Records a P_inst sample into the logarithmic histogram.
    ///
    /// # Arguments
    ///
    /// * `p_inst` - Instantaneous flicker perceptibility value to bin.
    pub fn add_sample(&mut self, p_inst: f32) {
        if p_inst <= 0.0 { return; }
        let clamped = p_inst.clamp(FLICKER_MIN_P, FLICKER_MAX_P);
        // Logarithmic bin mapping
        let norm_log = (clamped / FLICKER_MIN_P).ln() / (FLICKER_MAX_P / FLICKER_MIN_P).ln();
        let bin_idx = ((norm_log * FLICKER_BINS as f32).floor() as usize).min(FLICKER_BINS - 1);
        self.histogram[bin_idx] += 1;
        self.total_samples += 1;
    }

    /// Converts a histogram bin index to its representative P_inst value (bin center).
    ///
    /// # Arguments
    ///
    /// * `bin_idx` - Histogram bin index in [0, FLICKER_BINS).
    ///
    /// # Returns
    ///
    /// The P_inst value at the bin center on the logarithmic scale.
    fn bin_to_p_inst(bin_idx: usize) -> f32 {
        let frac = (bin_idx as f32 + 0.5) / FLICKER_BINS as f32;
        FLICKER_MIN_P * (FLICKER_MAX_P / FLICKER_MIN_P).powf(frac)
    }

    /// Returns the P_inst value exceeded for the given percentage of the time.
    ///
    /// # Arguments
    ///
    /// * `percent` - Percentage of observation time (e.g. 50 for P50).
    ///
    /// # Returns
    ///
    /// The P_inst value at the requested percentile, or 0.0 if no samples were recorded.
    pub fn get_exceeded_percentile(&self, percent: f32) -> f32 {
        if self.total_samples == 0 { return 0.0; }
        let target_count = (self.total_samples as f32 * (percent / 100.0)).round() as u32;
        let mut accum = 0;

        // Iterate backwards from highest bin to lowest
        for bin in (0..FLICKER_BINS).rev() {
            accum += self.histogram[bin];
            if accum >= target_count {
                return Self::bin_to_p_inst(bin);
            }
        }
        Self::bin_to_p_inst(0)
    }

    /// Calculates the Short-Term Flicker Severity Pst per IEC 61000-4-15.
    ///
    /// # Returns
    ///
    /// The weighted Pst value, or 0.0 if fewer than `FLICKER_PST_MIN_SAMPLES` samples were collected.
    pub fn calculate_pst(&self) -> f32 {
        if self.total_samples < FLICKER_PST_MIN_SAMPLES { return 0.0; }

        let p_0_1 = self.get_exceeded_percentile(0.1);
        let p_1   = self.get_exceeded_percentile(1.0);
        let p_3   = self.get_exceeded_percentile(3.0);
        let p_10  = self.get_exceeded_percentile(10.0);
        let p_50  = self.get_exceeded_percentile(50.0);

        let sum_sq = 0.0314 * p_0_1
            + 0.0525 * p_1
            + 0.0657 * p_3
            + 0.2800 * p_10
            + 0.0800 * p_50;

        sum_sq.max(0.0).sqrt()
    }
}

/// Calculates the Long-Term Flicker Severity Plt over 12 10-minute Pst values (2 hours).
///
/// # Arguments
///
/// * `pst_12_samples` - Array of 12 short-term Pst values.
///
/// # Returns
///
/// The Plt value computed as the cube root of the mean of the cubed Pst values.
pub fn calculate_plt(pst_12_samples: &[f32; 12]) -> f32 {
    let mut sum_cube = 0.0;
    for &pst in pst_12_samples.iter() {
        sum_cube += pst.max(0.0).powi(3);
    }
    (sum_cube / 12.0).cbrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies `calculate_plt` for constant and varying Pst sample sets.
    #[test]
    fn test_plt_calculation() {
        let pst_samples = [1.0; 12];
        let plt = calculate_plt(&pst_samples);
        assert!((plt - 1.0).abs() < 1e-4);

        let pst_varying = [1.0, 1.2, 0.8, 1.1, 0.9, 1.0, 1.3, 0.7, 1.0, 1.1, 0.9, 1.0];
        let plt_var = calculate_plt(&pst_varying);
        assert!(plt_var > 0.9 && plt_var < 1.3);
    }
}
