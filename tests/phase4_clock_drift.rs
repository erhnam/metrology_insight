// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

//! Phase 4 — Clock Drift Test Bench (§7.11)
//!
//! Simulates the ESP32 crystal drift over a 24-hour period and verifies that
//! the TimeModel drift correction keeps accumulated error below ±0.5 seconds.

use metrology_insight::types::TimeModel;

/// Tests the core drift correction algorithm over a simulated 24-hour period.
///
/// Initializes a `TimeModel` at boot, injects a known crystal ppm error, and
/// verifies that `ktime_to_utc` keeps the corrected time within tolerance of
/// true UTC across the whole interval.
///
/// # Arguments
///
/// * `crystal_ppm` - Crystal drift in parts per million (positive = fast clock).
/// * `tolerance_s` - Maximum allowed drift-corrected error in seconds.
///
/// # Panics
///
/// Panics if the maximum drift error over the simulated 24 hours exceeds the
/// tolerance.
fn run_drift_test(crystal_ppm: f64, tolerance_s: f64) {
    let duration_s = 24.0 * 3600.0; // 24 hours
    let sample_interval_s = 1.0;     // evaluate every 1 second
    let samples = (duration_s / sample_interval_s) as u64;

    // True UTC at boot (ns)
    let true_utc_at_boot_ns: u64 = 1_700_000_000_000_000_000; // ~2023-11-14

    // Simulate ktime at boot = true UTC at boot (they agree at t=0)
    let ktime_at_boot_ns = true_utc_at_boot_ns;

    // Crystal drift factor: if crystal runs +20 ppm, system clock advances
    // 1.000020 seconds per true second.
    let clock_rate = 1.0 + crystal_ppm / 1_000_000.0;

    // Recalibration would compute: drift_factor = delta_true_utc / delta_monotonic
    // For a crystal running fast (+ppm): more monotonic ticks than true UTC elapsed
    // drift_factor = 1.0 / clock_rate
    let expected_drift_factor = (1.0 / clock_rate) as f32;

    // Initialize TimeModel as if recalibrated at boot
    let mut tm = TimeModel::init_from_system(true_utc_at_boot_ns, ktime_at_boot_ns);
    // After recalibration, drift_factor would be adjusted. Simulate this.
    // We recalibrate at t=0 with a "perfect" NTP fix:
    tm.recalibrate(true_utc_at_boot_ns, ktime_at_boot_ns);
    // After this, drift_factor should be ~1.0 (since both agree at t=0).
    // We need to inject the known drift. Let's set it directly:
    tm.drift_factor = expected_drift_factor;

    let mut max_error_s = 0.0_f64;

    for step in 0..=samples {
        let elapsed_s = step as f64 * sample_interval_s;
        let elapsed_ns = (elapsed_s * 1e9) as u64;

        // True UTC at this instant
        let true_utc_ns = true_utc_at_boot_ns + elapsed_ns;

        // Simulate ktime (monotonic counter driven by the drifting crystal)
        let ktime_elapsed_ns = (elapsed_s * 1e9 * clock_rate) as u64;
        let ktime_ns = ktime_at_boot_ns + ktime_elapsed_ns;

        // TimeModel converts ktime → drift-corrected UTC
        let corrected_ns = tm.ktime_to_utc(ktime_ns);

        // Error = corrected - true UTC
        let error_s = if corrected_ns >= true_utc_ns {
            (corrected_ns - true_utc_ns) as f64 / 1e9
        } else {
            -((true_utc_ns - corrected_ns) as f64 / 1e9)
        };

        max_error_s = max_error_s.max(error_s.abs());
    }

    assert!(
        max_error_s <= tolerance_s,
        "Drift error {:.4}s exceeds tolerance {:.2}s at {} ppm",
        max_error_s, tolerance_s, crystal_ppm
    );
}

/// Verifies that with no crystal drift the drift correction error stays
/// within 1 ms over 24 hours.
///
/// # Panics
///
/// Panics if the drift error exceeds 1 ms.
#[test]
fn test_no_drift() {
    run_drift_test(0.0, 0.001);
}

/// Verifies drift correction keeps the error below 0.5 s for a crystal running
/// 10 ppm fast.
///
/// # Panics
///
/// Panics if the drift error exceeds 0.5 s.
#[test]
fn test_plus_10_ppm() {
    run_drift_test(10.0, 0.5);
}

/// Verifies drift correction keeps the error below 0.5 s for a crystal running
/// 10 ppm slow.
///
/// # Panics
///
/// Panics if the drift error exceeds 0.5 s.
#[test]
fn test_minus_10_ppm() {
    run_drift_test(-10.0, 0.5);
}

/// Verifies drift correction keeps the error below 0.5 s for a crystal running
/// 20 ppm fast.
///
/// # Panics
///
/// Panics if the drift error exceeds 0.5 s.
#[test]
fn test_plus_20_ppm() {
    run_drift_test(20.0, 0.5);
}

/// Verifies drift correction keeps the error below 0.5 s for a crystal running
/// 20 ppm slow.
///
/// # Panics
///
/// Panics if the drift error exceeds 0.5 s.
#[test]
fn test_minus_20_ppm() {
    run_drift_test(-20.0, 0.5);
}

/// Verifies drift correction keeps the error below 0.5 s for a crystal running
/// 50 ppm fast.
///
/// # Panics
///
/// Panics if the drift error exceeds 0.5 s.
#[test]
fn test_plus_50_ppm() {
    run_drift_test(50.0, 0.5);
}

/// Verifies drift correction keeps the error below 0.5 s for a crystal running
/// 50 ppm slow.
///
/// # Panics
///
/// Panics if the drift error exceeds 0.5 s.
#[test]
fn test_minus_50_ppm() {
    run_drift_test(-50.0, 0.5);
}

/// Verifies that recalibrating the time model at the 12-hour mark keeps the
/// post-recalibration error below 10 ms and the overall 24-hour error below
/// 1.5 s for a 30 ppm crystal.
///
/// # Panics
///
/// Panics if the post-recalibration error is not below 10 ms or the maximum
/// error is not below 1.5 s.
#[test]
fn test_recalibrate_at_12h() {
    let crystal_ppm = 30.0;
    let clock_rate = 1.0 + crystal_ppm / 1_000_000.0;
    let true_utc_at_boot_ns: u64 = 1_700_000_000_000_000_000;
    let ktime_at_boot_ns = true_utc_at_boot_ns;
    let mut tm = TimeModel::init_from_system(true_utc_at_boot_ns, ktime_at_boot_ns);

    let mut max_error_s = 0.0_f64;
    let mut first_after_recalib_s = 0.0_f64;

    for hour in 0..24 {
        let elapsed_s = hour as f64 * 3600.0;
        let elapsed_ns = (elapsed_s * 1e9) as u64;
        let true_utc_ns = true_utc_at_boot_ns + elapsed_ns;
        let ktime_elapsed_ns = (elapsed_s * 1e9 * clock_rate) as u64;
        let ktime_ns = ktime_at_boot_ns + ktime_elapsed_ns;

        if hour == 12 {
            tm.recalibrate(true_utc_ns, ktime_ns);
        }

        let corrected_ns = tm.ktime_to_utc(ktime_ns);
        let error_s = if corrected_ns >= true_utc_ns {
            (corrected_ns - true_utc_ns) as f64 / 1e9
        } else {
            -((true_utc_ns - corrected_ns) as f64 / 1e9)
        };
        max_error_s = max_error_s.max(error_s.abs());
        if hour == 13 { first_after_recalib_s = error_s.abs(); }
    }

    assert!(
        first_after_recalib_s < 0.01,
        "Post-recalibration error {:.6}s too large",
        first_after_recalib_s
    );
    assert!(
        max_error_s < 1.5,
        "Max error {:.4}s unexpectedly high",
        max_error_s
    );
}
