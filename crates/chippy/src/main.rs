#![warn(clippy::all, clippy::nursery, clippy::pedantic)]

use eframe::{App, CreationContext};
use crate::app::ChippyApp;

mod app;
mod worker_thread;

#[macro_use]
extern crate log;

fn build_app (_cc: &CreationContext) -> Result<Box<dyn App>, Box<dyn std::error::Error + Send + Sync>> {
    let server = "ws://localhost:8080".to_string();

    Ok(Box::new(ChippyApp::new(server)?))
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Trace).expect("Failed to enable logging");

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = document
            .get_element_by_id("chippy_canvas_id")
            .expect("Failed to find chippy_canvas_id")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("chippy_canvas_id was not a HtmlCanvasElement");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(build_app)
            )
            .await;

        // Remove the loading text and spinner:
        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}