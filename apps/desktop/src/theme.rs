use eframe::egui;

pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(30, 120, 220);

pub fn apply_visuals(ctx: &egui::Context, dark: bool) {
    let mut visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.widgets.active.bg_fill = ACCENT;
    visuals.widgets.hovered.bg_fill = ACCENT.gamma_multiply(0.85);
    visuals.widgets.open.bg_fill = ACCENT.gamma_multiply(0.75);
    visuals.selection.bg_fill = ACCENT.gamma_multiply(0.35);
    visuals.hyperlink_color = ACCENT;
    visuals.window_fill = if dark {
        egui::Color32::from_rgb(28, 28, 32)
    } else {
        egui::Color32::from_rgb(248, 249, 252)
    };
    visuals.panel_fill = if dark {
        egui::Color32::from_rgb(32, 32, 38)
    } else {
        egui::Color32::from_rgb(255, 255, 255)
    };
    ctx.set_visuals(visuals);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(12.0, 10.0);
        s.spacing.button_padding = egui::vec2(10.0, 6.0);
        s.spacing.window_margin = egui::Margin::same(16);
    });
}
