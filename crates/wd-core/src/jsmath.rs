//! `Math.sin` and friends. In the browser these are the engine's own functions, so results match the
//! original game bit for bit; Rust's own implementations differ from V8's in the last digit for some inputs.
#[cfg(target_arch = "wasm32")]
mod imp {
    #[link(wasm_import_module = "env")]
    extern "C" {
        fn js_sin(x: f64) -> f64;
        fn js_cos(x: f64) -> f64;
        fn js_tan(x: f64) -> f64;
        fn js_acos(x: f64) -> f64;
        fn js_atan(x: f64) -> f64;
        fn js_atan2(y: f64, x: f64) -> f64;
        fn js_pow(x: f64, y: f64) -> f64;
        fn js_random() -> f64;
    }
    pub fn sin(x: f64) -> f64 { unsafe { js_sin(x) } }
    pub fn cos(x: f64) -> f64 { unsafe { js_cos(x) } }
    pub fn tan(x: f64) -> f64 { unsafe { js_tan(x) } }
    pub fn acos(x: f64) -> f64 { unsafe { js_acos(x) } }
    pub fn atan(x: f64) -> f64 { unsafe { js_atan(x) } }
    pub fn atan2(y: f64, x: f64) -> f64 { unsafe { js_atan2(y, x) } }
    pub fn pow(x: f64, y: f64) -> f64 { unsafe { js_pow(x, y) } }
    pub fn random() -> f64 { unsafe { js_random() } }
}
#[cfg(not(target_arch = "wasm32"))]
mod imp {
    // fdlibm ports (what V8 uses for `Math.sin` and friends), not the platform's libm
    pub fn sin(x: f64) -> f64 { libm::sin(x) }
    pub fn cos(x: f64) -> f64 { libm::cos(x) }
    pub fn tan(x: f64) -> f64 { libm::tan(x) }
    pub fn acos(x: f64) -> f64 { libm::acos(x) }
    pub fn atan(x: f64) -> f64 { libm::atan(x) }
    pub fn atan2(y: f64, x: f64) -> f64 { libm::atan2(y, x) }
    pub fn pow(x: f64, y: f64) -> f64 { libm::pow(x, y) }
    pub fn random() -> f64 { 0.5 }
}
pub use imp::*;

/// `Math.hypot(a, b)` the way V8 computes it (scaled Kahan sum), not libm's `hypot`.
pub fn hypot2(a: f64, b: f64) -> f64 {
    if a.is_infinite() || b.is_infinite() { return f64::INFINITY; }
    if a.is_nan() || b.is_nan() { return f64::NAN; }
    let (x, y) = (a.abs(), b.abs());
    let max = x.max(y);
    if max == 0.0 { return 0.0; }
    let mut sum = 0.0;
    let mut compensation = 0.0;
    for v in [x, y] {
        let n = v / max;
        let summand = n * n - compensation;
        let preliminary = sum + summand;
        compensation = (preliminary - sum) - summand;
        sum = preliminary;
    }
    sum.sqrt() * max
}
