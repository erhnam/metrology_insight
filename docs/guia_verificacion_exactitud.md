# Practical Guide for Verifying Energy Measurement Accuracy

## 1. Required Equipment

- **Signal generator** — arbitrary waveform generator capable of producing sine waves with controlled amplitude, phase, and frequency for voltage and current channels.
- **Accurate reference meter** — class 0.05 or better reference energy meter used as ground truth (`Eref`).
- **Temperature chamber** — to characterise drift over the operating temperature range (e.g. –10 °C to +55 °C).

## 2. Test Setup

Connect the signal generator's voltage output to the meter's voltage input and the generator's current output to the meter's current input. For polyphase meters, repeat for each phase. Synchronise the reference meter on the same signals. Set nominal voltage `Un` (e.g. 230 V) and nominal frequency (50 Hz or 60 Hz). All tests below assume `Un` and `fn` unless otherwise noted.

## 3. Test Points (IEC 62053-21 Table 3)

| Point | PF       | Current level    | Class 1 limit |
|-------|----------|------------------|---------------|
| 1     | 1.0      | 0.05 In          | ±1.5 %        |
| 2     | 1.0      | 0.1 In           | ±1.0 %        |
| 3     | 1.0      | 0.5 In           | ±1.0 %        |
| 4     | 1.0      | In               | ±1.0 %        |
| 5     | 1.0      | Imax             | ±1.0 %        |
| 6     | 0.5 ind  | 0.1 In           | ±1.5 %        |
| 7     | 0.5 ind  | 0.2 In           | ±1.0 %        |
| 8     | 0.5 ind  | 0.5 In           | ±1.0 %        |
| 9     | 0.5 ind  | In               | ±1.0 %        |
| 10    | 0.8 cap  | 0.5 In           | ±1.0 %        |
| 11    | 0.8 cap  | In               | ±1.0 %        |
| 12    | 1.0      | In (1 ph loaded) | ±2.0 %        |

> Limits above are for **Class 1** meters per IEC 62053-21 Table 3. For Class 0.5 or Class 2 adjust limits accordingly.

## 4. Procedure for Each Point

1. Configure the signal generator to output the voltage and current waveforms specified by the test point.
2. Let the meter stabilise (minimum 10 s at the test condition).
3. Run for **N cycles** (e.g. 200 cycles at 50 Hz = 4 s). The integration time should be at least 10 s per IEC 62053-21 for formal testing; use `MIN_CYCLES = 1000` (20 s @ 50 Hz) for certification-grade runs.
4. Record the energy measured by the meter under test (`Emeas`) and the energy recorded by the reference meter (`Eref`).
5. Compute the percentage error:

```
error % = (Emeas - Eref) / Eref * 100
```

6. Compare `|error %|` against the Table 3 limit. If `|error %|` < limit, the point passes.

## 5. Using the Existing Test Infrastructure

The crate provides two main entry points in `metrology_insight/src/accuracy_test.rs`:

### Single-phase test

```rust
use metrology_insight::accuracy_test::run_accuracy_test;

let result = run_accuracy_test(
    v_rms: f32,   // e.g. 230.0
    i_rms: f32,   // e.g. 5.0 (In)
    pf: f32,      // e.g. 1.0, 0.5, -0.8 (negative = capacitive)
    freq: f32,    // e.g. 50.0
    cycles: u32,  // e.g. 200
);
```

The returned `AccuracyTestResult` contains:

| Field            | Type  | Description              |
|------------------|-------|--------------------------|
| `v_rms`          | f32   | Test voltage             |
| `i_rms`          | f32   | Test current             |
| `pf`             | f32   | Power factor             |
| `freq`           | f32   | Frequency                |
| `cycles`         | u32   | Number of cycles         |
| `energy_ref_wh`  | f64   | Reference energy (Wh)    |
| `energy_meas_wh` | f64   | Measured energy (Wh)     |
| `error_pct`      | f64   | Error in percent         |

### Polyphase test

```rust
use metrology_insight::accuracy_test::{run_polyphase_accuracy_test, PhaseTestPoint};

let phases = [
    PhaseTestPoint { v_rms: 230.0, i_rms: 5.0, pf: 1.0 },
    PhaseTestPoint { v_rms: 230.0, i_rms: 5.0, pf: 1.0 },
    PhaseTestPoint { v_rms: 230.0, i_rms: 5.0, pf: 1.0 },
];
let result = run_polyphase_accuracy_test(phases, 50.0, 200);
```

### Waveform generation helpers

- `generate_cycle(v_rms, i_rms, pf, freq, fs)` — pure sine V and I at a given power factor.
- `generate_cycle_with_harmonics(v_rms, i_rms, freq, fs)` — pure sine V, I with 3rd/5th/7th harmonics (20%/10%/5%).
- `generate_half_wave_cycle(v_rms, i_rms, pf, freq, fs)` — half-wave rectified I for DC component testing.

## 6. Test Report Template

| V (V) | I (A) | PF     | Freq (Hz) | Cycles | Eref (Wh) | Emeas (Wh) | Error (%) | Limit (%) | Pass/Fail |
|-------|-------|--------|-----------|--------|-----------|------------|-----------|-----------|-----------|
| 230.0 | 0.25  | 1.0    | 50        | 200    | 0.319     | 0.320      | +0.31     | ±1.5      | Pass      |
| 230.0 | 0.50  | 1.0    | 50        | 200    | 0.639     | 0.641      | +0.31     | ±1.0      | Pass      |
| 230.0 | 2.50  | 1.0    | 50        | 200    | 3.194     | 3.204      | +0.31     | ±1.0      | Pass      |
| 230.0 | 5.00  | 1.0    | 50        | 200    | 6.389     | 6.409      | +0.31     | ±1.0      | Pass      |
| 230.0 | 10.00 | 1.0    | 50        | 200    | 12.778    | 12.819     | +0.32     | ±1.0      | Pass      |
| 230.0 | 0.50  | 0.5    | 50        | 200    | 0.319     | 0.320      | +0.31     | ±1.5      | Pass      |
| 230.0 | 1.00  | 0.5    | 50        | 200    | 0.639     | 0.641      | +0.31     | ±1.0      | Pass      |
| 230.0 | 2.50  | 0.5    | 50        | 200    | 3.194     | 3.204      | +0.31     | ±1.0      | Pass      |
| 230.0 | 5.00  | 0.5    | 50        | 200    | 6.389     | 6.409      | +0.31     | ±1.0      | Pass      |
| 230.0 | 2.50  | 0.8    | 50        | 200    | 5.111     | 5.127      | +0.31     | ±1.0      | Pass      |
| 230.0 | 5.00  | 0.8    | 50        | 200    | 10.222    | 10.255     | +0.32     | ±1.0      | Pass      |

## 7. Running the Automated Test Suite

The existing integration test `table3_error_limits.rs` implements all Table 3 points above. To execute it:

```bash
cargo test -p metrology_insight --target x86_64-unknown-linux-gnu
```

To run a single test point (e.g. point 4, `in_pf1`):

```bash
cargo test -p metrology_insight --target x86_64-unknown-linux-gnu -- point4
```

For verbose output showing individual errors:

```bash
cargo test -p metrology_insight --target x86_64-unknown-linux-gnu -- --nocapture
```

### Reference files

| File | Purpose |
|------|---------|
| `metrology_insight/src/accuracy_test.rs` | Core test functions (`run_accuracy_test`, `run_polyphase_accuracy_test`, waveform generators) |
| `metrology_insight/tests/table3_error_limits.rs` | Integration tests covering IEC 62053-21 Table 3 points 1–12 |
