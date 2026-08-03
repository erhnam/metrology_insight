//! Half-cycle RMS (Urms) measurement with sum-of-squares accumulation.
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

/// Half-cycle RMS tracker using sum-of-squares accumulation.
#[derive(Debug, Clone, Copy, Default)]
pub struct UrmsHalfCycle {
    sum_sq_prev: f32,
    count_prev: f32,
    sum_sq_curr: f32,
    count_curr: f32,
    pub urms: f32,
}

impl UrmsHalfCycle {
    /// Create a new half-cycle RMS tracker initialised to zero.
    ///
    /// # Returns
    ///
    /// A new [`UrmsHalfCycle`] in its default (all-zero) state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Accumulate a sample into the current half-cycle sum of squares.
    ///
    /// # Arguments
    ///
    /// * `sample` — Sample value to accumulate.
    pub fn process_sample(&mut self, sample: f32) {
        self.sum_sq_curr += sample * sample;
        self.count_curr += 1.0;
    }

    /// Close the current half-cycle and finalise the RMS value.
    ///
    /// Once at least `min_samples` samples have been accumulated, computes the
    /// RMS over the previous and current half-cycle sums and resets the
    /// half-cycle buffers.
    ///
    /// # Arguments
    ///
    /// * `min_samples` — Minimum number of samples required to finalise a cycle.
    ///
    /// # Returns
    ///
    /// `true` when a half-cycle was finalised, `false` when there are still not
    /// enough accumulated samples.
    pub fn half_cycle_trigger(&mut self, min_samples: f32) -> bool {
        if self.count_curr < min_samples {
            return false;
        }
        let total_sum_sq = self.sum_sq_prev + self.sum_sq_curr;
        let total_count = self.count_prev + self.count_curr;
        
        if total_count > 0.0 {
            self.urms = (total_sum_sq / total_count).sqrt();
        }
        
        self.sum_sq_prev = self.sum_sq_curr;
        self.count_prev = self.count_curr;
        self.sum_sq_curr = 0.0;
        self.count_curr = 0.0;
        true
    }
}
