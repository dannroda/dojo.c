// UniFFI bindings for Dojo
// Multi-language bindings (Swift, Kotlin, Python) using UniFFI
pub mod uniffi;

// Re-export all UniFFI types at crate root for scaffolding
pub use uniffi::*;

// Include the generated UniFFI scaffolding only for non-WASM targets
#[cfg(not(target_arch = "wasm32"))]
::uniffi::include_scaffolding!("dojo");
