use wasm_bindgen::prelude::*;

mod engine;
pub use engine::W3gpuEngine;

#[wasm_bindgen(start)]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Debug);
}
