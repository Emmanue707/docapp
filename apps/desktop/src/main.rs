#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod icon;
mod recent;
mod state;

use app::DocApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let icon = icon::window_icon();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Waghitech Docapp")
            .with_icon(icon),
        ..Default::default()
    };
    eframe::run_native(
        "Waghitech Docapp",
        options,
        Box::new(|cc| {
            setup_theme(cc);
            Ok(Box::new(DocApp::default()))
        }),
    )
}

fn setup_theme(cc: &eframe::CreationContext<'_>) {
    let dark = cc.egui_ctx.style().visuals.dark_mode;
    cc.egui_ctx.set_visuals(if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    });
    if std::env::var("DOCAPP_DARK").ok().as_deref() == Some("1") {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
    } else if std::env::var("DOCAPP_DARK").ok().as_deref() == Some("0") {
        cc.egui_ctx.set_visuals(egui::Visuals::light());
    }
}
