//! Embedded-first electrical metrology DSP library (IEC 61000-4-30 Class S, IEC 62053-21).
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod channel_map;
pub mod detector;
pub mod energy;
pub mod filters;
pub mod flicker;
pub mod harmonics;
pub mod phase;
pub mod power;
pub mod print;
pub mod pll;
pub mod processing;
pub mod resampling;
pub mod signal;
pub mod types;
pub mod urms;
pub mod voltage_current;
pub mod windowing;
pub mod events;
pub mod unbalance;
pub mod rvc;
pub mod oscillography;

#[cfg(feature = "alloc")]
pub mod accuracy_test;
#[cfg(feature = "alloc")]
pub mod generate_signal;

pub use types::*;
pub use signal::*;
pub use phase::*;
pub use power::*;
pub use energy::*;
pub use harmonics::*;
pub use events::*;
pub use unbalance::*;
pub use rvc::*;
pub use oscillography::*;
pub use print::*;
