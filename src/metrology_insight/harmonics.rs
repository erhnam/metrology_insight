use core::f64::consts::PI;
use num_complex::Complex;
use realfft::RealFftPlanner;

pub const FFT_RESOLUTION: usize = 128;

// Función para resampleo lineal
pub fn resample_signal(signal: &[f64], new_len: usize) -> Vec<f64> {
    let n = signal.len();
    let step = n as f64 / new_len as f64;
    let mut resampled = Vec::with_capacity(new_len);

    for i in 0..new_len {
        let pos = i as f64 * step;
        let idx0 = pos.floor() as usize % n;
        let idx1 = (idx0 + 1) % n;
        let fraction = pos - idx0 as f64;

        let y0 = signal[idx0];
        let y1 = signal[idx1];
        resampled.push(y0 + (y1 - y0) * fraction);
    }
    resampled
}

// Cálculo de THD y armónicos
fn calculate_harmonics_and_thd(magnitudes: &[f64], fundamental_mag: f64, fundamental_bin: usize) -> ([f64; 21], f64) {
    let mut harmonics = [0.0; 21];
    let mut harmonic_power_sum = 0.0;

    // Solo consideramos armónicos impares (1°, 3°, 5°, ... 41°)
    for i in 0..harmonics.len() {
        let harmonic_order = 2 * i + 1;
        let harmonic_bin = harmonic_order * fundamental_bin;

        if let Some(mag) = magnitudes.get(harmonic_bin) {
            // Almacenar armónico como porcentaje del fundamental
            harmonics[i] = (*mag / fundamental_mag) * 100.0;

            // Excluir el fundamental (i=0) para el cálculo del THD
            if i > 0 {
                harmonic_power_sum += mag.powi(2);
            }
        }
    }

    // Calcular THD: sqrt(suma de potencias armónicas) / fundamental
    let mut thd = (harmonic_power_sum.sqrt() / fundamental_mag) * 100.0;
    thd = 20.0 * (thd / 100.0).log10();
    (harmonics, thd)
}

// Función principal para cálculo de armónicos y THD
pub fn compute_harmonics_and_thd(signal: &mut [f64], freq: f64, fs: f64) -> Option<([f64; 21], f64)> {
    // (armónicos, THD)
    let sample_rate = fs; // frecuencia de muestreo, deberías pasarla como parámetro si cambia
    let bin_freq = sample_rate / signal.len() as f64;

    // 1. Preprocesamiento de la señal
    apply_window(signal);
    remove_mean(signal);

    // 2. Realizar FFT
    let spectrum = compute_fft(signal)?;

    // 3. Calcular magnitudes
    let mut magnitudes = vec![0.0; spectrum.len()];
    for (i, bin) in spectrum.iter().enumerate() {
        magnitudes[i] = (bin.re * bin.re + bin.im * bin.im).sqrt();
    }

    // Debug magnitudes
    //for (i, mag) in magnitudes.iter().enumerate().take(20) {
    //    println!("Bin {} → Mag: {:.3}", i, mag);
    //}

    // 4. Identificar bin fundamental (50/60 Hz)
    let fundamental_bin: usize = (freq / bin_freq).round() as usize;
    let fundamental_mag = magnitudes.get(fundamental_bin).copied()?;

    // Protección por si el espectro está vacío
    if fundamental_mag < f64::EPSILON {
        return None;
    }

    // 6. Calcular THD y armónicos impares
    let (harmonics, thd) = calculate_harmonics_and_thd(&magnitudes, fundamental_mag, fundamental_bin);

    Some((harmonics, thd))
}

// Aplicar ventana de Hann para reducir fugas espectrales
fn apply_window(signal: &mut [f64]) {
    let n = signal.len();
    for i in 0..n {
        let window = 0.5 * (1.0 - (2.0 * PI * i as f64 / (n as f64 - 1.0)).cos());
        signal[i] *= window;
    }
}

// Remover componente DC
fn remove_mean(signal: &mut [f64]) {
    let mean = signal.iter().sum::<f64>() / signal.len() as f64;
    for sample in signal.iter_mut() {
        *sample -= mean;
    }
}

// Implementación compatible std/no_std para FFT
fn compute_fft(signal: &mut [f64]) -> Option<Vec<Complex<f64>>> {
    #[cfg(feature = "std")]
    {
        let mut planner = RealFftPlanner::<f64>::new();
        let r2c = planner.plan_fft_forward(signal.len());
        let mut spectrum = r2c.make_output_vec();
        let mut scratch = r2c.make_scratch_vec();

        r2c.process_with_scratch(&mut signal.to_vec(), &mut spectrum, &mut scratch)
            .ok()?;
        Some(spectrum)
    }

    #[cfg(not(feature = "std"))]
    {
        // Implementación con microfft para no_std
        use microfft::complex::cfft_128;

        if signal.len() != FFT_RESOLUTION {
            return None;
        }

        let mut complex_signal: [Complex<f32>; FFT_RESOLUTION] = [Complex::new(0.0, 0.0); FFT_RESOLUTION];
        for (i, &s) in signal.iter().enumerate() {
            complex_signal[i] = Complex::new(s as f32, 0.0);
        }

        let spectrum = cfft_128(&mut complex_signal);
        // Convert Vec<Complex<f32>> to Vec<Complex<f64>>
        let spectrum_f64: Vec<Complex<f64>> = spectrum
            .iter()
            .map(|c| Complex::new(c.re as f64, c.im as f64))
            .collect();
        Some(spectrum_f64)
    }
}
