#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("Waghitech Docapp"),
        ..Default::default()
    };
    eframe::run_native(
        "Waghitech Docapp",
        options,
        Box::new(|_cc| Ok(Box::new(DesktopApp))),
    )
}

#[derive(Default)]
struct DesktopApp;

impl eframe::App for DesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Waghitech Docapp");
            ui.label(format!("v{}", docsuite_core::VERSION));
            if ui.button("Open").clicked() {}
        });
    }
}
