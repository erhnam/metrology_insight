# Compliance Status

> Standards support (pre-compliance): This library implements measurement algorithms derived from several IEC standards. It is intended for development, evaluation, and pre-compliance testing. It has not been certified or validated as fully compliant with the referenced standards.

## Status Legend

| Status | Meaning |
|--------|---------|
| **Algorithm implementation** | Method present and functional; no conformance/accuracy testing performed. |
| **Partial implementation** | Method present but does not fully implement the normative requirements; gaps listed. |
| **Pre-compliance** | Meets applicable limits under software simulation only; not tested on hardware/reference instrumentation. |
| **N/R** | Not required for Class S. |

## IEC 61000-4-30:2021 Class S

| Clause | Parameter | Status | Notes / Evidence |
|--------|-----------|--------|------------------|
| 5.1 | Power frequency | Partial implementation | PLL tracking + 10 s sliding average (`src/pll.rs`); no 10-min aggregation; ±50 mHz accuracy not verified. |
| 5.2 | Supply voltage magnitude | Partial implementation | 10-cycle RMS (`src/signal.rs:357-364`) and Urms(½) (`src/urms.rs`); 20–120 % Udin range and ±0.5 % accuracy not verified. |
| 5.3 | Flicker (Pst, Plt) | Partial implementation | IEC 61000-4-15 Blocks 1–4 with P_inst computed in realtime (`src/flicker.rs`); P_st classifier and P_lt helper provided but not wired into the realtime pipeline. |
| 5.4 | Voltage dips and swells | Partial implementation | Urms(½) detector with IEC-typical thresholds (90 %/110 % + 1 % hysteresis) (`src/events.rs`); no sliding reference Usr (§5.4.4). |
| 5.5 | Voltage interruptions | Partial implementation | Detection via Urms(½) < 10 % (`src/events.rs`). |
| 5.7 | Supply voltage unbalance | Partial implementation | Fortescue symmetrical components u2 + u0 (`src/unbalance.rs`); ±0.3 % accuracy and 1–5 % range not verified. |
| 5.8 | Voltage harmonics | Partial implementation | 512-point RFFT + THD up to 50th order (`src/harmonics.rs`); uses a **1-cycle** window while IEC 61000-4-7 (Class II) requires **10-cycle** windows; single-bin method (no subgroups). |
| 5.9 | Voltage interharmonics | Algorithm implementation | 49-band Goertzel accumulator; Class S method is left to the manufacturer (SBM) (`src/harmonics.rs`). |
| 5.11 | Rapid voltage changes (RVC) | Partial implementation | Urms(½) sliding-window state machine (`src/rvc.rs`); window length is an approximation of the §5.11 method. |
| 5.13 | Current (incl. current unbalance) | N/R for Class S | RMS, harmonics and current unbalance (§5.13.6) present (`src/unbalance.rs`, `src/voltage_current.rs`). |
| 4.6 / 4.7 | Quality flags | Partial implementation | `Q_FLAG_PLL_UNSETTLED`, `Q_FLAG_SYNC_INCONSISTENT`, `Q_FLAG_OUT_OF_RANGE`, `Q_FLAG_EVENT_MARKED` (`src/types.rs:511-515`, `src/signal.rs:335-342`). |

## IEC 62053-21 (2nd Ed.) — Static Meters for AC Active Energy

| Aspect | Status | Notes |
|--------|--------|-------|
| 4-quadrant bi-directional active energy | Algorithm implementation | i128 micro-joule accumulators (`src/energy.rs`). |
| Class 1 error limits (Table 3) | Pre-compliance | Verified in software simulation only — see `docs/informe_pruebas_tipo.md`; not tested against a reference meter or temperature chamber. |
| Class 1 influence quantities (Table 4) | Pre-compliance | Same note as above. |
| No-load / starting current / repeatability | Pre-compliance | Software simulation only — see `tests/phase3_type_tests.rs`. |

## IEC 61000-4-15 — Flickermeter

| Aspect | Status |
|--------|--------|
| Blocks 1–4 (input adaptor, demodulation, 35 Hz / weighting filters, smoothing) | Algorithm implementation |
| P_inst | Implemented in realtime |
| P_st / P_lt | Partial — classifier / helper provided as library functions; not wired into the realtime output |

## IEC 62053-23 — Static Meters for AC Reactive Energy

| Aspect | Status |
|--------|--------|
| Reactive energy (Q1–Q4 quadrant decomposition) | Algorithm implementation; pre-compliance |

## Platform Support

- **`no_std` + `alloc`**: the crate is `no_std` compatible but **requires a target
  with an `alloc` allocator** (ARM Cortex-M, ESP32/Xtensa, RISC-V with a heap).
  The core heap-allocates via `Box` on initialization; allocator-less targets
  (e.g. bare `riscv32i` without a heap) are not supported. This is a platform
  requirement, not a standards-compliance gap.
- The `alloc` feature is optional and only gates the test/simulation helpers
  (`accuracy_test`, `generate_signal`); the core pipeline builds with
  `--no-default-features` on any allocator-bearing target.

## Gaps Toward Certification

1. **Harmonics**: switch from a 1-cycle window to 10-cycle windows + subgroup method per IEC 61000-4-7 Class II (§5.8).
2. **Flicker**: wire P_st (10-min) and P_lt (2-h) accumulation into the realtime pipeline (§5.3).
3. **Aggregation**: implement 10-min / 2-h time aggregation per §4.5 for Class S quantities.
4. **Accuracy verification**: validate against normative limits (frequency ±50 mHz, voltage ±0.5 % over 20–120 % Udin, flicker 0.4–4.0 Pst, unbalance ±0.3 %, harmonics range).
5. **Hardware validation**: test against a reference meter (class 0.05 or better) on a controlled bench (temperature, reference conditions) per IEC 62053-21 test procedures.

## Evidence

- Sources: `src/pll.rs`, `src/signal.rs`, `src/urms.rs`, `src/flicker.rs`, `src/events.rs`, `src/rvc.rs`, `src/harmonics.rs`, `src/unbalance.rs`, `src/energy.rs`, `src/types.rs`
- Integration tests: `tests/table3_error_limits.rs`, `tests/table4_influence.rs`, `tests/phase3_type_tests.rs`
- Reports: `docs/informe_pruebas_tipo.md`, `docs/guia_verificacion_exactitud.md`
