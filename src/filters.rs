//! Fixed-size moving-average filter implementation.
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

/// Fixed-size moving-average filter.
///
/// Maintains a circular buffer of the last `N` values and returns the
/// running arithmetic mean on each [`push`](MovingAverage::push).
///
/// # Type parameters
/// * `N` — Number of taps (const generic, must be > 0).
pub struct MovingAverage<const N: usize> {
    buffer: [f32; N],
    index: usize,
    count: usize,
    sum: f32,
}

impl<const N: usize> MovingAverage<N> {
    /// Create a new moving-average filter initialised to zero.
    ///
    /// # Returns
    ///
    /// A new [`MovingAverage`] with all buffer entries, the index, count, and
    /// sum set to zero.
    pub fn new() -> Self {
        Self {
            buffer: [0.0; N],
            index: 0,
            count: 0,
            sum: 0.0,
        }
    }
}

impl<const N: usize> Default for MovingAverage<N> {
    /// Returns a `MovingAverage` initialised to zero via [`new`](MovingAverage::new).
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> MovingAverage<N> {
    /// Push a new sample and return the current moving average.
    ///
    /// Until the buffer is filled (`N` samples), the average is
    /// computed over fewer samples.
    ///
    /// # Arguments
    ///
    /// * `value` — New sample value.
    ///
    /// # Returns
    ///
    /// The current moving average.
    pub fn push(&mut self, value: f32) -> f32 {
        if self.count < N {
            self.count += 1;
        } else {
            self.sum -= self.buffer[self.index];
        }
        self.buffer[self.index] = value;
        self.sum += value;
        self.index = (self.index + 1) % N;
        self.sum / self.count as f32
    }
}
