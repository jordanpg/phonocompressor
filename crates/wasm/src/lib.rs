//! Browser binding for phonocompressor (wasm-bindgen).
//!
//! The JavaScript side fetches `phono.bin` and passes its bytes in; this crate only marshals
//! between JavaScript and [`phono_core`], which does the work. Build it with
//! `mise run wasm:build` (`wasm-bindgen --target web`).

use wasm_bindgen::prelude::*;

/// The binding's crate version and the data file format it reads, e.g. `0.1.0 (data v2)`.
#[wasm_bindgen]
pub fn version() -> String {
    format!(
        "{} (data v{})",
        env!("CARGO_PKG_VERSION"),
        phono_core::data::FORMAT_VERSION
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_names_the_data_format() {
        let v = version();
        let data = format!(" (data v{})", phono_core::data::FORMAT_VERSION);
        assert!(v.ends_with(&data), "{v}");
    }
}
