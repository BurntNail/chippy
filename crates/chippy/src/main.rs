#![warn(clippy::all, clippy::nursery, clippy::pedantic)]

use eframe::NativeOptions;
use crate::app::ChippyApp;

mod app;
mod worker_thread;

fn main() {
    eframe::run_native(
        "Chippy",
        NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(ChippyApp::new())))
    ).unwrap()
}