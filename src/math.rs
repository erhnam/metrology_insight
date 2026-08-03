//! Float math helpers available in both `std` and `no_std` builds.
//!
//! Uses the standard-library float methods when the `std` feature is enabled
//! and falls back to `libm` equivalents otherwise, so the crate keeps a single
//! code path across both build configurations.
//!
// Copyright © 2026 Francisco Arcos.
// SPDX-License-Identifier: Apache-2.0

/// Sine of an f32 value in radians.
pub fn sin(x: f32) -> f32 {
    #[cfg(feature = "std")]
    {
        x.sin()
    }
    #[cfg(not(feature = "std"))]
    {
        libm::sinf(x)
    }
}

/// Square root of an f32 value.
pub fn sqrt(x: f32) -> f32 {
    #[cfg(feature = "std")]
    {
        x.sqrt()
    }
    #[cfg(not(feature = "std"))]
    {
        libm::sqrtf(x)
    }
}

/// Square root of an f64 value.
pub fn sqrt64(x: f64) -> f64 {
    #[cfg(feature = "std")]
    {
        x.sqrt()
    }
    #[cfg(not(feature = "std"))]
    {
        libm::sqrt(x)
    }
}

/// Nearest integer (rounded half away from zero) as an f32.
pub fn round(x: f32) -> f32 {
    #[cfg(feature = "std")]
    {
        x.round()
    }
    #[cfg(not(feature = "std"))]
    {
        libm::roundf(x)
    }
}

/// Arc-cosine of an f32 value in radians.
pub fn acos(x: f32) -> f32 {
    #[cfg(feature = "std")]
    {
        x.acos()
    }
    #[cfg(not(feature = "std"))]
    {
        libm::acosf(x)
    }
}

/// Natural logarithm of an f32 value.
pub fn ln(x: f32) -> f32 {
    #[cfg(feature = "std")]
    {
        x.ln()
    }
    #[cfg(not(feature = "std"))]
    {
        libm::logf(x)
    }
}

/// Largest integer less than or equal to `x` as an f32.
pub fn floor(x: f32) -> f32 {
    #[cfg(feature = "std")]
    {
        x.floor()
    }
    #[cfg(not(feature = "std"))]
    {
        libm::floorf(x)
    }
}

/// Integer part of `x` (truncation toward zero) as an f32.
pub fn trunc(x: f32) -> f32 {
    #[cfg(feature = "std")]
    {
        x.trunc()
    }
    #[cfg(not(feature = "std"))]
    {
        libm::truncf(x)
    }
}

/// Fractional part of `x` as an f32.
pub fn fract(x: f32) -> f32 {
    #[cfg(feature = "std")]
    {
        x.fract()
    }
    #[cfg(not(feature = "std"))]
    {
        x - libm::truncf(x)
    }
}

/// `x` raised to the power `n` as an f32.
pub fn powf(x: f32, n: f32) -> f32 {
    #[cfg(feature = "std")]
    {
        x.powf(n)
    }
    #[cfg(not(feature = "std"))]
    {
        libm::powf(x, n)
    }
}

/// `x` raised to the integer power `n` as an f32.
pub fn powi(x: f32, n: i32) -> f32 {
    #[cfg(feature = "std")]
    {
        x.powi(n)
    }
    #[cfg(not(feature = "std"))]
    {
        libm::powf(x, n as f32)
    }
}

/// `x` raised to the integer power `n` as an f64.
pub fn powi64(x: f64, n: i32) -> f64 {
    #[cfg(feature = "std")]
    {
        x.powi(n)
    }
    #[cfg(not(feature = "std"))]
    {
        libm::pow(x, n as f64)
    }
}

/// Cube root of an f32 value.
pub fn cbrt(x: f32) -> f32 {
    #[cfg(feature = "std")]
    {
        x.cbrt()
    }
    #[cfg(not(feature = "std"))]
    {
        libm::cbrtf(x)
    }
}
