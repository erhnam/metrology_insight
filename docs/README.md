# metrology_insight — Documentation

> **Standards support (pre-compliance):** This library implements measurement algorithms derived from several IEC standards. It is intended for development, evaluation, and pre-compliance testing. It has not been certified or validated as fully compliant with the referenced standards. See [COMPLIANCE_STATUS.md](./COMPLIANCE_STATUS.md) for the per-clause status.

## Files

| Document | Description |
|-----------|-------------|
| [API_REFERENCE.md](./API_REFERENCE.md) | Complete reference of all modules, types, functions and constants (English) |
| [API_REFERENCE_ES.md](./API_REFERENCE_ES.md) | Complete reference of all modules, types, functions and constants (Spanish) |
| [COMPLIANCE_STATUS.md](./COMPLIANCE_STATUS.md) | Per-clause standards compliance status (pre-compliance) |

## Quick Start

```rust
use metrology_insight::{MetrologyInsight, MetrologyInsightConfig, SystemMode};

// Minimum setup — all other fields use IEC defaults
let mut config = MetrologyInsightConfig::default();
config.adc_samples_seconds = 8000.0;
config.adc_samples_per_cycle = 160.0;
config.avg_sec = 160.0 / 8000.0; // 1 cycle averaging

let mut insight = MetrologyInsight::new(config);
// Optional: syncs nominal_voltage across event_config + flicker simultaneously
insight.set_nominal_voltage(230.0);

let active = SystemMode::ThreePhase4Wire.active_phases(); // 4

// Deposit ADC samples into socket.phases[i].{voltage,current}
insight.process_and_update_metrics(active);

let v_rms  = insight.socket.phases[0].voltage.rms;
let freq   = insight.socket.phases[0].voltage.pll_state.freq_10s; // 10s avg EN 61000-4-30
let p_kw   = insight.socket.power_metrics_total.real_power / 1000.0;
let u2_pct = insight.socket.unbalance_metrics.u2_neg_ratio_pct;   // Negative unbalance
```

## Integration Strategy for Embedded Systems

To avoid data loss (missed ADC samples) during intensive mathematical processing (FFT, THD, RMS), an asynchronous or multi-core architecture is recommended:

1. **Acquisition (Core 0 or High Priority Task)**:
   - Use asynchronous polling or interrupts for the ADC `DRDY` pin.
   - Read SPI data and accumulate into a buffer (e.g., 160 samples at 50 Hz / 8 kSPS).
   - Send the full buffer via an inter-thread channel without blocking acquisition.

2. **DSP Processing (Core 1 or Medium Priority Task)**:
   - Receive buffers from the channel and transfer them to `insight.socket.phases[i]`.
   - Call `insight.process_and_update_metrics()`.
   - Publish metrics (MQTT, REST). Decoupling acquisition from DSP guarantees sample continuity.

## Standards Compliance

Pre-compliance status — not certified. See [COMPLIANCE_STATUS.md](./COMPLIANCE_STATUS.md) for the per-clause breakdown.

| Standard | Status | Application |
|----------|--------|-----------|
| IEC 61000-4-30:2021 Class S | Partial implementation | Quality flags, synchronous resampling, 10-cycle RMS, 10 s frequency, Fortescue unbalance, Dips/Swells/RVC |
| IEC 62053-21 (2nd Ed.) | Algorithm / pre-compliance | Static meters for AC active energy (Classes 1/2 limits), 4-Quadrant energy metering — simulation only |
| IEC 61000-4-15 | Algorithm implementation | Flickermeter Blocks 1–4 ($P_{\text{inst}}$ realtime; $P_{\text{st}}$ / $P_{\text{lt}}$ library helpers) — incorporated by reference via IEC 61000-4-30 §5.3 |
| IEC 62053-23 | Algorithm implementation | Static meters for AC reactive energy (Q1–Q4 quadrant decomposition) |
| EN 50160 | Partial implementation | Quality events: Dip, Swell, Interruption, RVC threshold limits |

## Runtime Configuration

After building `MetrologyInsight::new(config)`, all configuration values can be
hot-modified and propagated to sub-components:

```rust
// Change nominal voltage (syncs event_config + flicker automatically)
insight.set_nominal_voltage(120.0); // American market

// Modify any parameter and propagate manually
insight.config.pll.kp = 0.003;
insight.config.flicker.rms_tc_seconds = 30.0;
insight.apply_config(); // Propagates to FlickerMeter and other sub-components
```

## Changelog

| Commit | Change |
|--------|--------|
| `6b603ef` | 10 s frequency integration (IEC 61000-4-30 §5.1), fixed initial flicker transient, Fortescue unbalance protection with PLL lock |
| `fcbdd00` | Critical fix capacitive/inductive detection: rising zero-crossing + signed angle (φ = θI − θV). Unit test added. |
| `2f481c2` | Full refactor: 27 named constants, new structs `FlickerConfig`, `PhaseConfig`, `SignalConfig`; extended `PllConfig` and `RvcConfig`; `apply_config()` and `set_nominal_voltage()` methods |
| `77165df` | `GridFrequency` propagation → `nominal_freq` + PLL bounds from firmware `Measurements::apply_config()` |
