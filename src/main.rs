#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use egui::IconData;
use ping_monitor::PingMonitorApp;
use eframe::egui;

fn main() -> eframe::Result {
    env_logger::init();

    let app = PingMonitorApp::new();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([450.0, 600.0])
            .with_resizable(false)
            .with_always_on_top()
            .with_icon(IconData::default()),
        ..Default::default()
    };
    eframe::run_native(
        "Ping Monitor",
        options,
        Box::new(move |_cc| {
            Ok(Box::new(app))
        }),
    )
}
