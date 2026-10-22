//! WASM-only browser media experiment. Native UI has no browser media imports.
#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub use browser::start;
