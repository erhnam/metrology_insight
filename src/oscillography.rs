//! Oscillography capture window constants (pre/post-trigger cycles).
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

use serde::{Serialize, Deserialize};

pub const PRE_TRIGGER_CYCLES: usize = 10;
pub const POST_TRIGGER_CYCLES: usize = 50;
pub const SAMPLES_PER_CYCLE: usize = 160; // 8000 SPS / 50 Hz

pub const PRE_TRIGGER_SAMPLES: usize = PRE_TRIGGER_CYCLES * SAMPLES_PER_CYCLE; // 1600
pub const POST_TRIGGER_SAMPLES: usize = POST_TRIGGER_CYCLES * SAMPLES_PER_CYCLE; // 8000
pub const TOTAL_SAMPLES: usize = PRE_TRIGGER_SAMPLES + POST_TRIGGER_SAMPLES; // 9600

pub const MAX_CHANNELS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerSource {
    Manual,
    Dip(u8),          // Phase index
    Swell(u8),        // Phase index
    Interruption(u8), // Phase index
    Rvc(u8),          // Phase index
    Alarm(u8),        // Alarm rule index
}

impl TriggerSource {
    /// Returns the short string name of this trigger source.
    ///
    /// # Returns
    ///
    /// A static string such as "MANUAL", "DIP" or "ALARM".
    pub fn as_str(&self) -> &'static str {
        match self {
            TriggerSource::Manual => "MANUAL",
            TriggerSource::Dip(_) => "DIP",
            TriggerSource::Swell(_) => "SWELL",
            TriggerSource::Interruption(_) => "INTERRUPTION",
            TriggerSource::Rvc(_) => "RVC",
            TriggerSource::Alarm(_) => "ALARM",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OscillographyState {
    Idle,
    Armed,
    Capturing,
    Ready,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OscillographyHeader {
    pub id: heapless::String<32>,
    pub trigger_source: TriggerSource,
    pub timestamp_ns: u64,
    pub phase_mode: u8, // PhaseMode enum value from system
    pub sample_rate_hz: u32,
    pub num_channels: u8,
    pub total_samples: u32,
}

/// Buffer for a single channel. Uses a circular buffer for pre-trigger and a sequential buffer for post-trigger.
pub struct ChannelBuffer {
    pub pre_trigger: [f32; PRE_TRIGGER_SAMPLES],
    pub post_trigger: [f32; POST_TRIGGER_SAMPLES],
    pub pre_write_ptr: usize,
    pub post_write_ptr: usize,
}

impl Default for ChannelBuffer {
    /// Returns a `ChannelBuffer` with all samples zeroed and both write pointers at 0.
    fn default() -> Self {
        Self {
            pre_trigger: [0.0; PRE_TRIGGER_SAMPLES],
            post_trigger: [0.0; POST_TRIGGER_SAMPLES],
            pre_write_ptr: 0,
            post_write_ptr: 0,
        }
    }
}

impl ChannelBuffer {
    /// Clears all buffered samples and resets both write pointers to 0.
    pub fn reset(&mut self) {
        self.pre_write_ptr = 0;
        self.post_write_ptr = 0;
        self.pre_trigger.fill(0.0);
        self.post_trigger.fill(0.0);
    }

    /// Writes one pre-trigger sample, wrapping around when the buffer is full.
    ///
    /// # Arguments
    ///
    /// * `val` - Sample to store in the pre-trigger ring buffer.
    #[inline(always)]
    pub fn feed_pre(&mut self, val: f32) {
        self.pre_trigger[self.pre_write_ptr] = val;
        self.pre_write_ptr = (self.pre_write_ptr + 1) % PRE_TRIGGER_SAMPLES;
    }

    /// Writes one post-trigger sample, if the post-trigger buffer is not yet full.
    ///
    /// # Arguments
    ///
    /// * `val` - Sample to store in the post-trigger buffer.
    ///
    /// # Returns
    ///
    /// `true` once the post-trigger buffer is full (capture completed).
    #[inline(always)]
    pub fn feed_post(&mut self, val: f32) -> bool {
        if self.post_write_ptr < POST_TRIGGER_SAMPLES {
            self.post_trigger[self.post_write_ptr] = val;
            self.post_write_ptr += 1;
            self.post_write_ptr == POST_TRIGGER_SAMPLES
        } else {
            true
        }
    }

    /// Reads all samples in chronological order: oldest pre-trigger samples first, then
    /// post-trigger samples.
    ///
    /// # Arguments
    ///
    /// * `dest` - Destination slice receiving the ordered samples.
    pub fn read_all(&self, dest: &mut [f32]) {
        let mut idx = 0;
        // 1. Read pre-trigger buffer starting from the oldest sample (which is at the current write pointer)
        for i in 0..PRE_TRIGGER_SAMPLES {
            let src_idx = (self.pre_write_ptr + i) % PRE_TRIGGER_SAMPLES;
            dest[idx] = self.pre_trigger[src_idx];
            idx += 1;
        }
        // 2. Read post-trigger buffer up to the current write pointer (should be fully written)
        let post_limit = self.post_write_ptr.min(POST_TRIGGER_SAMPLES);
        for i in 0..post_limit {
            dest[idx] = self.post_trigger[i];
            idx += 1;
        }
        // If not fully written, pad the rest with last value or 0.0
        if idx < dest.len() {
            let last_val = if idx > 0 { dest[idx - 1] } else { 0.0 };
            dest[idx..].fill(last_val);
        }
    }
}

/// The core Oscillography manager running inside the metrology_insight pipeline.
pub struct OscillographyManager {
    pub channels: [ChannelBuffer; MAX_CHANNELS],
    pub state: OscillographyState,
    pub trigger_source: Option<TriggerSource>,
    pub trigger_timestamp_ns: u64,
    pub phase_mode: u8,
    pub active_channels: u8,
}

impl Default for OscillographyManager {
    /// Returns an `OscillographyManager` with all channels cleared, `Idle` state and 8 active channels.
    fn default() -> Self {
        Self {
            channels: [
                ChannelBuffer::default(), ChannelBuffer::default(),
                ChannelBuffer::default(), ChannelBuffer::default(),
                ChannelBuffer::default(), ChannelBuffer::default(),
                ChannelBuffer::default(), ChannelBuffer::default(),
            ],
            state: OscillographyState::Idle,
            trigger_source: None,
            trigger_timestamp_ns: 0,
            phase_mode: 0,
            active_channels: 8,
        }
    }
}

impl OscillographyManager {
    /// Creates an oscillography manager in the `Idle` state.
    ///
    /// # Returns
    ///
    /// A new `OscillographyManager`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Arms the oscillography manager for a capture on the given phase mode and channel count.
    ///
    /// # Arguments
    ///
    /// * `phase_mode` - PhaseMode enum value from the system.
    /// * `num_channels` - Number of channels to capture.
    pub fn arm(&mut self, phase_mode: u8, num_channels: u8) {
        self.state = OscillographyState::Armed;
        self.trigger_source = None;
        self.trigger_timestamp_ns = 0;
        self.phase_mode = phase_mode;
        self.active_channels = num_channels;
        for ch in &mut self.channels {
            ch.reset();
        }
    }

    /// Triggers a capture immediately when armed, entering the capturing state.
    ///
    /// # Arguments
    ///
    /// * `source` - Source that caused the trigger.
    /// * `now_ns` - Trigger timestamp in nanoseconds.
    pub fn force_trigger(&mut self, source: TriggerSource, now_ns: u64) {
        if self.state == OscillographyState::Armed {
            self.state = OscillographyState::Capturing;
            self.trigger_source = Some(source);
            self.trigger_timestamp_ns = now_ns;
            for ch in &mut self.channels {
                ch.post_write_ptr = 0;
            }
        }
    }

    /// Feeds one sample per channel into the manager; buffers pre-trigger samples while armed
    /// and post-trigger samples while capturing.
    ///
    /// # Arguments
    ///
    /// * `samples` - One sample per channel for this time step.
    /// * `_now_ns` - Current timestamp (reserved for future use).
    ///
    /// # Returns
    ///
    /// `true` when a capture completed on this call.
    #[inline(always)]
    pub fn feed_sample(&mut self, samples: &[f32; MAX_CHANNELS], _now_ns: u64) -> bool {
        match self.state {
            OscillographyState::Armed => {
                // Buffer pre-trigger samples continuously
                for i in 0..(self.active_channels as usize).min(MAX_CHANNELS) {
                    self.channels[i].feed_pre(samples[i]);
                }
                false
            }
            OscillographyState::Capturing => {
                // Fill post-trigger buffer
                let mut completed = true;
                for i in 0..(self.active_channels as usize).min(MAX_CHANNELS) {
                    let ch_done = self.channels[i].feed_post(samples[i]);
                    completed = completed && ch_done;
                }

                if completed {
                    self.state = OscillographyState::Ready;
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}
