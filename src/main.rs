#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use egui::IconData;
use ping_monitor::PingMonitorApp;
use eframe::egui;

fn load_icon() -> IconData {
    let icon_bytes = include_bytes!("../icons/icon-128.png");
    IconData {
        rgba: load_icon_rgba(icon_bytes),
        width: 128,
        height: 128,
    }
}

fn load_icon_rgba(icon_bytes: &[u8]) -> Vec<u8> {
    let image = image::load_from_memory(icon_bytes).unwrap().to_rgba8();
    image.into_raw()
}

fn main() -> eframe::Result {
    env_logger::init();

    let app = PingMonitorApp::new();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 450.0])
            .with_resizable(false)
            .with_always_on_top()
            .with_icon(load_icon()),
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
