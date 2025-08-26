#![warn(clippy::all, clippy::nursery, clippy::pedantic)]

use eframe::{App, CreationContext};
use crate::app::ChippyApp;

mod app;
mod worker_thread;

fn build_app (_cc: &CreationContext) -> Result<Box<dyn App>, Box<dyn std::error::Error + Send + Sync>> {
    let server = if cfg!(target_arch = "wasm32") {
        "fishand.fly.dev:8080"
    } else {
        "localhost:8080"
    };
    
    Ok(Box::new(ChippyApp::new(server)))
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let options = eframe::NativeOptions::default();
    
    color_eyre::install().expect("unable to install colour eyre :(");

    eframe::run_native(
        "Chippy",
        options,
        Box::new(build_app)
    ).unwrap()
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;
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