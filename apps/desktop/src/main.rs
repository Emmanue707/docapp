#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod home;
mod icon;
mod modal;
mod parse_util;
mod recent;
mod screenshot;
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
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Waghitech Docapp",
        options,
        Box::new(|cc| {
            let dark = match std::env::var("DOCAPP_DARK").ok().as_deref() {
                Some("1") => true,
                Some("0") => false,
                _ => false,
            };
            theme::apply_visuals(&cc.egui_ctx, dark);
            let mut app = DocApp::default();
            app.dark_mode = dark;
            if let Ok(p) = std::env::var("DOCAPP_OPEN") {
                app.open_path(std::path::PathBuf::from(p));
            }
            if let Ok(p) = std::env::var("DOCAPP_SCREENSHOT")
                && p.ends_with(".png")
            {
                app.screenshot_path = Some(std::path::PathBuf::from(p));
            }
            Ok(Box::new(app))
        }),
    )
}
