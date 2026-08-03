# MetrologyInsight

[![Rust](https://img.shields.io/badge/rust-2021-brightgreen.svg)](https://www.rust-lang.org/)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![Pre-compliance](https://img.shields.io/badge/IEC%2061000--4--30%3A2021-Class%20S%20(pre--compliance)-orange.svg)]()
[![Pre-compliance](https://img.shields.io/badge/IEC%2062053--21-Active%20Energy%20(pre--compliance)-purple.svg)]()

> **Standards support (pre-compliance):** This library implements measurement algorithms derived from several IEC standards. It is intended for development, evaluation, and pre-compliance testing. It has not been certified or validated as fully compliant with the referenced standards. See [docs/COMPLIANCE_STATUS.md](./docs/COMPLIANCE_STATUS.md) for the per-clause status.

High-performance, embedded-first electrical metrology DSP library written in Rust. Implements measurement algorithms derived from **IEC 61000-4-30:2021 Class S** (Power Quality Measurement Methods) and **IEC 62053-21** (Static Meters for AC Active Energy).

Engineered for microcontrollers (e.g., ESP32-S3 / Xtensa, ARM Cortex-M) with zero hardware dependencies, fully compatible with `no_std` + `alloc`.

---

## Key Features

- **Multi-Phase Topology Support**: Configurable 1 to 4 channels (Single-phase, Single-phase with neutral, 3-Phase 3-Wire, 3-Phase 4-Wire).
- **True RMS Processing**: Fractional cycle correction for ultra-precise Voltage and Current RMS measurements.
- **High-Inertia Digital PLL**: Grid frequency tracking (45 Hz – 65 Hz) with 10-second sliding frequency averages (IEC 61000-4-30 §5.1).
- **Synchronous Resampling**: Coherent windowing via linear-interpolation resampling to an integer number of cycles for leakage-free spectral analysis.
- **FFT & Harmonics**: 512-point RFFT processing up to the 50th harmonic order + total harmonic distortion (THD-V and THD-I).
- **Comprehensive Power Metrics**: Active (W), Reactive (VAR), Apparent (VA), and Power Factor (PF) per phase and 3-phase totals.
- **Bi-Directional 4-Quadrant Energy**: High-resolution `i128` micro-joule (µJ) accumulators for active/reactive energy across Q1–Q4 (IEC 62053-21).
- **Signed Phase Angles & Direction**: Directional power flow determination (Inductive, Capacitive, In-Phase) with signed phase shift calculations ($\varphi = \theta_I - \theta_V$).
- **Fortescue Unbalance Ratios**: Symmetrical component analysis (Zero $U_0$ and Negative $U_2$ sequence ratios) for voltage and current.
- **Flickermeter Implementation**: Implementation of IEC 61000-4-15 Blocks 1–4 via IEC 61000-4-30 §5.3 (SOS Butterworth 35 Hz filter, weighting filter, instantaneous flicker $P_{\text{inst}}$ in realtime; $P_{\text{st}}$ / $P_{\text{lt}}$ classifiers available as library functions — pre-compliance).
- **Power Quality Event Detection**: Automated tracking of Voltage Dips, Swells, Interruptions, and Rapid Voltage Changes (RVC).
- **IEC Quality Flags**: Real-time diagnostic flags (`PLL_UNSETTLED`, `SYNC_INCONSISTENT`, `OUT_OF_RANGE`, `EVENT_MARKED`).
- **Hardware-Agnostic**: Works with any ADC front-end (ADS131M08, MCP3913, internal ADCs) by accepting normalized physical values.

---

## Standards Compliance

Pre-compliance status — not certified. See [docs/COMPLIANCE_STATUS.md](./docs/COMPLIANCE_STATUS.md) for the per-clause breakdown.

| Standard | Status | Description / Implementation |
|----------|--------|------------------------------|
| **IEC 61000-4-30:2021 Class S** | Partial implementation | Frequency (§5.1), 10-cycle RMS (§5.2), flicker Blocks 1–4 (§5.3), Dips/Swells/Interruptions (§5.4/5.5), Fortescue unbalance (§5.7), harmonics (§5.8, 1-cycle window — gap vs. 10-cycle requirement), interharmonics (§5.9, SBM), RVC (§5.11), quality flags |
| **IEC 62053-21 (2nd Ed.)** | Algorithm implementation / pre-compliance | Static meters for AC active energy; 4-Quadrant bi-directional energy metering targeting Classes 1/2 limits — verified in software simulation only |
| **IEC 61000-4-15** | Algorithm implementation | Flickermeter Blocks 1–4 ($P_{\text{inst}}$ realtime; $P_{\text{st}}$ / $P_{\text{lt}}$ library helpers) — incorporated by reference via IEC 61000-4-30 §5.3 |
| **IEC 62053-23** | Algorithm implementation | Static meters for AC reactive energy (Q1–Q4 quadrant decomposition) |
---

## Resource Requirements & Memory Footprint

> **`no_std` + `alloc` contract**: `no_std` builds require a target with an
> `alloc` allocator (e.g. ARM Cortex-M, ESP32/Xtensa, RISC-V with a heap).
> The core heap-allocates on initialization via `Box` (see table below);
> targets without any allocator (e.g. bare `riscv32i` without a heap) are not
> supported. The `alloc` feature only adds optional test/simulation helpers and
> is not required for the core pipeline.

### 1. Code Footprint (Flash / `.text` Section)

| Mode | Build Command | Flash Size (`.text`) | Key Characteristics |
|------|---------------|----------------------|---------------------|
| `no_std` + `alloc` | `--no-default-features --features alloc` | **~48 – 65 KB** | Ultra-lean embedded DSP pipeline (`microfft`, `libm`, zero OS dependencies) |
| `std` *(default)* | `--default-features` | **~75 – 95 KB** | Includes `realfft` planner, standard `log` hooks, and string formatters |

### 2. Heap Memory Footprint (RAM / Dynamic Allocations)

Allocated safely via `Box` on initialization:
- **Base Socket Allocation**: `MetrologyInsightSocket` (352 Bytes)
- **Per-Phase Data Struct**: `Box<PhaseData>` (**4,224 Bytes / 4.125 KiB**)

| System Topology | Active Phases | Phase Data + Socket Heap | FFT Scratch Buffer (`FftCache`) | Total Heap RAM Required |
|-----------------|---------------|--------------------------|--------------------------------|-------------------------|
| **Single-Phase** | 1 Phase (CH0=V, CH1=I) | **4.47 KiB** (1 × 4.125 KiB + 352 B) | ~2.0 KiB (lazy allocation) | **~6.5 KiB** |
| **Single-Phase + Neutral** | 2 Phases (A + N) | **8.60 KiB** (2 × 4.125 KiB + 352 B) | ~2.0 KiB | **~10.6 KiB** |
| **3-Phase 3-Wire** | 3 Phases (A, B, C Delta) | **12.72 KiB** (3 × 4.125 KiB + 352 B) | ~2.0 KiB | **~14.7 KiB** |
| **3-Phase 4-Wire** | 4 Phases (A, B, C + N Wye) | **16.85 KiB** (4 × 4.125 KiB + 352 B) | ~2.0 KiB | **~18.9 KiB** |

### 3. Stack Memory Footprint (Thread Task Stack)

- **`MetrologyInsight` Instance Size**: **320 Bytes** on stack (holds pointers, configuration struct, and active phase counter).
- **DSP Processing Stack Usage**: **< 1.5 KiB** peak stack usage during `process_and_update_metrics()` calls (temporary stack frames for trapezoidal integration and zero-crossing calculation).

---

## Quick Start

Add `metrology_insight` to your `Cargo.toml`:

```toml
[dependencies]
metrology_insight = { version = "0.1.0", default-features = false, features = ["alloc"] }
```

### Basic Example

```rust
use metrology_insight::{MetrologyInsight, MetrologyInsightConfig, SystemMode};

fn main() {
    // 1. Configure for an 8000 Hz ADC sample rate (e.g., ADS131M08 @ OSR=1024)
    let mut config = MetrologyInsightConfig::default();
    config.adc_samples_seconds = 8000.0;
    config.adc_samples_per_cycle = 160.0; // 8000 Hz / 50 Hz
    config.avg_sec = 160.0 / 8000.0;      // 1-cycle EWMA smoothing

    let mut insight = MetrologyInsight::new(config);
    insight.set_nominal_voltage(230.0);    // Synchronizes event & flicker thresholds

    let active_phases = SystemMode::ThreePhase4Wire.active_phases(); // 4

    // 2. Feed ADC samples into insight.socket.phases[i].{voltage, current}
    // ...
    
    // 3. Process DSP pipeline
    insight.process_and_update_metrics(active_phases);

    // 4. Access real-time metrics
    let v_rms  = insight.socket.phases[0].voltage.rms;
    let freq   = insight.socket.phases[0].voltage.pll_state.freq_10s;
    let p_kw   = insight.socket.power_metrics_total.real_power / 1000.0;
    let thd_v  = insight.socket.phases[0].voltage.thd;
    let flags  = insight.socket.phases[0].voltage.quality_flags;
    let u2_pct = insight.socket.unbalance_metrics.u2_neg_ratio_pct;

    println!("L1 RMS: {:.2} V | Freq: {:.3} Hz | P_total: {:.2} kW", v_rms, freq, p_kw);
}
```

---

## Dynamic Runtime Configuration

```rust
// Update nominal voltage on the fly (automatically re-calibrates flicker and event limits)
insight.set_nominal_voltage(120.0); // US 120V Grid

// Tweak DSP parameters dynamically and re-apply
insight.config.pll.kp = 0.003;
insight.config.flicker.rms_tc_seconds = 30.0;
insight.apply_config();
```

---

## Architecture & Crate Features

`metrology_insight` is structured into zero-cost modular DSP components:

```
src/
├── types.rs           # Core data structures, configuration & socket metrics
├── processing.rs      # Main execution pipeline coordinator
├── pll.rs             # Digital PLL frequency tracking
├── resampling.rs      # Kaiser/Sinc synchronous resampler
├── harmonics.rs       # 512-point RFFT & harmonic analysis
├── flicker.rs         # IEC 61000-4-15 Flickermeter filters
├── power.rs           # Real, reactive, apparent power & PF calculations
├── energy.rs          # 4-Quadrant micro-joule energy accumulators
├── unbalance.rs       # Fortescue symmetrical sequence components
├── events.rs          # Dips, Swells, Interruptions & RVC detectors
└── rvc.rs             # Rapid Voltage Change detector
```

### Feature Flags

- `std` *(default)*: Enables standard library support and FFT hardware acceleration.
- `alloc`: Optional — adds the test/simulation helpers (`accuracy_test`, `generate_signal`) to `no_std` builds. The core library itself always requires a target with an `alloc` allocator (see [no_std contract](#no_std--alloc-contract)).

---

## Documentation

Comprehensive documentation of all functions, structs, and algorithms is available in:
- [`docs/API_REFERENCE.md`](./docs/API_REFERENCE.md)
- [`docs/COMPLIANCE_STATUS.md`](./docs/COMPLIANCE_STATUS.md) — per-clause standards compliance status

---

## License

`MetrologyInsight` is licensed under the **[Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0)**.

- **Permitted**: Free use, modification, and distribution, including for commercial purposes, subject to the terms of the license.
- Full license text: [`LICENSE.md`](./LICENSE.md)
- Copyright © 2026 Francisco Arcos.

### Professional Services

If you are integrating `MetrologyInsight` into a product and need support, custom feature development, metrology expertise, or certification assistance (e.g., IEC 61000-4-30, IEC 62053-21), feel free to reach out via GitHub Issues.
