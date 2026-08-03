# Energy Measurement Method

## 1. Overview

Energy is computed by accumulating power over time. Per computational cycle, real power (P) and reactive power (Q) are derived from the raw voltage and current samples of all active phases. The energy increment for that cycle is `power × elapsed_time`, converted to microjoules (µJ) and added to one of four quadrant accumulators depending on the sign of P and Q. At query time, the i128 µJ accumulators are converted to kWh via a constant factor.

Key source files:

- `metrology_insight/src/energy.rs` — quadrant allocation, accumulation, total aggregation
- `metrology_insight/src/power.rs` — real/reactive power computation per phase

---

## 2. Active Energy by Quadrant

`active_energy_by_quadrant()` in `energy.rs:16` handles active energy.

### 2.1 Real Power

`real_power_from_signals()` in `power.rs:9` computes:

```
P = Σ(v[i] × i[i]) / N
```

where `v[i]` and `i[i]` are the voltage and current samples in the real wave buffer.

### 2.2 Quadrant Allocation

The quadrant is determined by the signs of real power (`p_real`) and reactive power (`p_react`):

| Quadrant | P sign | Q sign | Condition              |
|----------|--------|--------|------------------------|
| Q1       | > 0    | ≥ 0    | Imported + inductive   |
| Q2       | < 0    | ≥ 0    | Exported + inductive   |
| Q3       | < 0    | < 0    | Exported + capacitive  |
| Q4       | > 0    | < 0    | Imported + capacitive  |

### 2.3 Accumulation

```
delta_uj = |P| × elapsed_time × W_SEC_TO_UJ
```

`W_SEC_TO_UJ = 1_000_000.0` converts watt-seconds to microjoules.

The delta is accumulated as `i128` in `q1_uj` / `q2_uj` / `q3_uj` / `q4_uj`.

### 2.4 Conversion to kWh

```
factor = JOULES_TO_KWH / W_SEC_TO_UJ
       = (1 / (3600 × 1000)) / 1_000_000
       = 1 / 3.6e12
```

The `f64` fields `q1`–`q4` are recomputed each cycle as `qN_uj × factor`.

---

## 3. Reactive Energy by Quadrant

`reactive_energy_by_quadrant()` in `energy.rs:38` follows the same pattern but uses reactive power:

```
delta_uj = |Q| × elapsed_time × W_SEC_TO_UJ
```

The quadrant rules differ slightly (based on `p_real` and `p_react` sign):

| Quadrant  | P sign | Q sign | Condition              |
|-----------|--------|--------|------------------------|
| Q1        | ≥ 0    | > 0    | Imported + inductive   |
| Q2        | < 0    | > 0    | Exported + inductive   |
| Q3        | < 0    | < 0    | Exported + capacitive  |
| Q4        | ≥ 0    | < 0    | Imported + capacitive  |

---

## 4. Total Energy Aggregation

`update_total_energy()` in `energy.rs:65` calls the quadrant functions, then computes aggregate metrics:

| Metric         | Formula              | Description                      |
|----------------|----------------------|----------------------------------|
| Active imported | Q1 + Q4              | Energy flowing into the load     |
| Active exported | Q2 + Q3              | Energy flowing back to the grid  |
| Active balance  | imported − exported  | Net energy                       |
| Reactive inductive | Q1 + Q3           | Inductive reactive energy        |
| Reactive capacitive | Q2 + Q4          | Capacitive reactive energy       |

See `ActiveEnergyMetrics::imported()` / `exported()` (`types.rs:553–563`) and `ReactiveEnergyMetrics::inductive()` / `capacitive()` (`types.rs:582–592`).

---

## 5. Noise Gate (No-Load Protection)

In `processing.rs:89–95`, energy accumulation is skipped when **all** active phases have current RMS below the noise threshold:

```
noise_threshold = ist_a × 0.4
```

Where `ist_a` is the rated transitional current from the meter standard values (`types.rs:226`).

This implements no-load threshold per **IEC 62053-21 §7.6**, preventing energy register creep under very low or zero load conditions.

---

## 6. Time Reference

Elapsed time per cycle is derived in `elapsed_time_seconds()` (`energy.rs:6`):

```
sample_duration = 1.0 / adc_samples_seconds
elapsed_time    = real_wave_len × sample_duration
```

`real_wave_len` is the number of samples captured in the current wave buffer (typically one mains cycle), and `adc_samples_seconds` is the ADC sampling rate.

---

## 7. Overflow Handling

All four quadrant accumulators are `i128` integers (µJ). The maximum representable value is roughly:

```
i128::MAX ≈ 1.7 × 10³¹ µJ ≈ 4.7 × 10¹⁸ kWh
```

For context, a meter measuring 100 kW continuously would need ~10¹³ years to overflow. In practice the accumulators will never overflow during the device's operational lifetime.

---

## 8. Key Function References

| Function                                 | File:Line      | Purpose                                |
|------------------------------------------|----------------|----------------------------------------|
| `active_energy_by_quadrant()`            | `energy.rs:16` | Allocates and accumulates active energy |
| `reactive_energy_by_quadrant()`          | `energy.rs:38` | Allocates and accumulates reactive energy |
| `update_energy_by_quadrant()`            | `energy.rs:60` | Calls both active and reactive quadrant updates |
| `update_total_energy()`                  | `energy.rs:65` | Quadrant updates + aggregate totals   |
| `real_power_from_signals()`              | `power.rs:9`   | Real power from raw V/I samples       |
| `update_power_metrics()`                 | `power.rs:53`  | Per-phase power + polyphase totals    |
| `elapsed_time_seconds()`                 | `energy.rs:6`  | Computes elapsed time per cycle        |
| Noise gate check                         | `processing.rs:89` | No-load protection gate            |
