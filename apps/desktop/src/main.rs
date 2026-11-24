#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod home;
mod icon;
mod modal;
mod parse_util;
mod recent;
mod sidebar;
mod state;
mod theme;
mod tools;
mod viewer_util;

use app::DocApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let icon = icon::window_icon();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Waghitech Docapp")
            .with_inner_size([1100.0, 720.0])
            .with_icon(icon),
        ..Default::default()
    };
    eframe::run_native(
        "Waghitech Docapp",
        options,
        Box::new(|cc| {
            let dark = cc.egui_ctx.style().visuals.dark_mode;
            if std::env::var("DOCAPP_DARK").ok().as_deref() == Some("1") {
                theme::apply_visuals(&cc.egui_ctx, true);
            } else if std::env::var("DOCAPP_DARK").ok().as_deref() == Some("0") {
                theme::apply_visuals(&cc.egui_ctx, false);
            } else {
                theme::apply_visuals(&cc.egui_ctx, dark);
            }
            Ok(Box::new(DocApp::default()))
        }),
    )
}
