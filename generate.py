import numpy as np
import matplotlib.pyplot as plt

# =========================================================================
# SIGNAL GENERATOR CONFIGURATION & CONSTANTS (RUST REPLICATE)
# =========================================================================

VPEAK = 325.26915       # Peak voltage (230.0 Vrms Phase-Neutral -> 398.37 Vrms Line-Line)
IPEAK = 70.71068        # Peak current (50.0 Arms)
IPHASE = -18.2       # Inductive phase shift (-18.2 deg -> PF ≈ 0.95)
SAMPLES_OFFSET = 12.0

NOISE_FREQ = 6000.0
NOISE_VPEAK_PERCENT = 0.002
NOISE_IPEAK_PERCENT = 0.005
NOISE_RANDOM_PERCENT = 0.001

FS = 8000.0           # Sampling frequency (Hz)
F = 49.98             # Grid frequency (Hz)
N_SAMPLES = 160       # 160 samples per cycle buffer

# ADC Scaling Factors (ADS131M08 24-bit Delta-Sigma)
ADC_FULL_SCALE_COUNTS = 8388608.0
VIN_TO_COUNTS = (ADC_FULL_SCALE_COUNTS / 1.2) / 410.09 # ~17047.016 LSB/V
AMPS_TO_COUNTS = (ADC_FULL_SCALE_COUNTS / 1.2) / (2000.0 / 100.0) # ~349525.3 LSB/A

ENABLE_HARMONICS = True

# 11 Realistic harmonics (Voltage & Current)
VOLTAGE_HARMONICS = [
    (3.0, 0.015), (5.0, 0.012), (7.0, 0.008), (9.0, 0.003),
    (11.0, 0.002), (13.0, 0.001), (15.0, 0.001), (17.0, 0.0005),
    (19.0, 0.0005), (21.0, 0.0002), (23.0, 0.0001)
]

CURRENT_HARMONICS = [
    (3.0, 0.065), (5.0, 0.045), (7.0, 0.025), (9.0, 0.012),
    (11.0, 0.008), (13.0, 0.005), (15.0, 0.003), (17.0, 0.002),
    (19.0, 0.001), (21.0, 0.001), (23.0, 0.0005)
]

# =========================================================================
# HELPER FUNCTIONS
# =========================================================================

def offset_rad(deg):
    return deg * 2.0 * np.pi / 360.0

def angle_rad(phase_deg, i):
    return offset_rad(phase_deg) + 2.0 * np.pi * F / FS * i

def gen_one_signal(phase_deg, peak, is_voltage):
    i_indices = np.arange(N_SAMPLES)
    a = angle_rad(phase_deg, i_indices)
    
    # Fundamental
    if is_voltage:
        sig = peak * np.sin(a)
    else:
        sig = peak * np.sin(a + offset_rad(IPHASE))

    # Harmonics
    if ENABLE_HARMONICS:
        harmonics = VOLTAGE_HARMONICS if is_voltage else CURRENT_HARMONICS
        for harm_order, perc in harmonics:
            freq = F * harm_order
            harm_peak = peak * perc
            harm_angle = offset_rad(phase_deg) + 2.0 * np.pi * freq / FS * i_indices
            if is_voltage:
                sig += harm_peak * np.sin(harm_angle)
            else:
                sig += harm_peak * np.sin(harm_angle + offset_rad(IPHASE))

    # Noise
    if NOISE_VPEAK_PERCENT > 0.0 and is_voltage:
        sig += peak * (NOISE_VPEAK_PERCENT * np.sin(2.0 * np.pi * NOISE_FREQ / FS * i_indices))
    if NOISE_IPEAK_PERCENT > 0.0 and not is_voltage:
        sig += peak * (NOISE_IPEAK_PERCENT * np.sin(2.0 * np.pi * NOISE_FREQ / FS * i_indices))
    if NOISE_RANDOM_PERCENT > 0.0:
        np.random.seed(42)
        noise = np.random.rand(N_SAMPLES)
        sig += peak * (noise - np.mean(noise)) / np.max(noise) * NOISE_RANDOM_PERCENT

    return sig

def to_i32(signal_float):
    return np.trunc(signal_float + SAMPLES_OFFSET).astype(np.int32)

# =========================================================================
# FFT & POWER METRICS CALCULATIONS (IEC Standard)
# =========================================================================

def calculate_metrics(signal_adc, conversion_factor):
    # Convert ADC LSB counts back to physical values (V or A)
    phys_signal = (signal_adc.astype(float) - SAMPLES_OFFSET) / conversion_factor
    
    # RMS Value
    rms = np.sqrt(np.mean(phys_signal**2))
    v_peak = np.max(np.abs(phys_signal))

    # FFT Calculation
    fft_vals = np.fft.rfft(phys_signal)
    fft_mag = np.abs(fft_vals) * (2.0 / N_SAMPLES)
    fft_mag[0] /= 2.0  # DC component fix

    # Búsqueda directa del pico fundamental en los primeros bins
    # Para F = 50Hz, FS = 8000Hz, N = 160 -> bin 1 es exactamente 50Hz (160 * 50 / 8000 = 1)
    fund_bin = 1  # Forzamos el bin 1 (50 Hz) para buffers sincronizados de 1 ciclo
    h1_mag = fft_mag[fund_bin]

    print(f"[DEBUG] fund_bin: {fund_bin} | h1_mag: {h1_mag:.3f} | RMS: {rms:.3f}")

    harmonics_mag = []
    thd_sum_sq = 0.0

    for order in range(2, 24):
        center_bin = order * fund_bin
        if center_bin < len(fft_mag):
            mag_k = fft_mag[center_bin]   # bin exacto, sin ventana
            ratio = mag_k / h1_mag
            harmonics_mag.append(ratio * 100.0)
            thd_sum_sq += ratio**2

    thd_pct = np.sqrt(thd_sum_sq) * 100.0
    return rms, v_peak, h1_mag, thd_pct, harmonics_mag

# =========================================================================
# MAIN EXECUTION
# =========================================================================

if __name__ == "__main__":
    v_peak_counts = VPEAK * VIN_TO_COUNTS
    i_peak_counts = IPEAK * AMPS_TO_COUNTS

    # Generate Phase A
    sig_v_a_counts = to_i32(gen_one_signal(0.0, v_peak_counts, True))
    sig_i_a_counts = to_i32(gen_one_signal(0.0, i_peak_counts, False))

    # Process metrics
    v_rms, v_peak, v_h1, thd_v, v_harmonics = calculate_metrics(sig_v_a_counts, VIN_TO_COUNTS)
    i_rms, i_peak, i_h1, thd_i, i_harmonics = calculate_metrics(sig_i_a_counts, AMPS_TO_COUNTS)

    print("=" * 60)
    print("           POWER QUALITY ANALYSIS (PYTHON SIMULATION)")
    print("=" * 60)
    print(f"Voltage RMS   : {v_rms:.3f} V   (Peak: {v_peak:.3f} V)")
    print(f"Current RMS   : {i_rms:.3f} A   (Peak: {i_peak:.3f} A)")
    print("-" * 60)
    print(f"Calculated THD-V : {thd_v:.3f} %  (Expected ~2.2%)")
    print(f"Calculated THD-I : {thd_i:.3f} %  (Expected ~8.5%)")
    print("-" * 60)
    print("Harmonic Spectrum (Voltage % of H1):")
    print(f"  H3 (150Hz) : {v_harmonics[1]:.3f}%")
    print(f"  H5 (250Hz) : {v_harmonics[3]:.3f}%")
    print(f"  H7 (350Hz) : {v_harmonics[5]:.3f}%")
    print("-" * 60)
    print("Harmonic Spectrum (Current % of H1):")
    print(f"  H3 (150Hz) : {i_harmonics[1]:.3f}%")
    print(f"  H5 (250Hz) : {i_harmonics[3]:.3f}%")
    print(f"  H7 (350Hz) : {i_harmonics[5]:.3f}%")
    print("=" * 60)