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
    let window = if dark {
        egui::Color32::from_rgb(28, 28, 32)
    } else {
        egui::Color32::from_rgb(248, 249, 252)
    };
    let panel = if dark {
        egui::Color32::from_rgb(40, 40, 46)
    } else {
        egui::Color32::from_rgb(255, 255, 255)
    };
    visuals.window_fill = window;
    visuals.panel_fill = panel;
    visuals.extreme_bg_color = window;
    visuals.faint_bg_color = panel;
    visuals.widgets.noninteractive.bg_fill = panel;
    visuals.widgets.inactive.bg_fill = panel;
    visuals.widgets.noninteractive.fg_stroke = if dark {
        egui::Stroke::new(1.0_f32, egui::Color32::from_gray(210))
    } else {
        egui::Stroke::new(1.0_f32, egui::Color32::from_gray(55))
    };
    visuals.widgets.inactive.fg_stroke = if dark {
        egui::Stroke::new(1.0_f32, egui::Color32::from_gray(220))
    } else {
        egui::Stroke::new(1.0_f32, egui::Color32::from_gray(35))
    };
    ctx.set_visuals(visuals);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(12.0, 10.0);
        s.spacing.button_padding = egui::vec2(10.0, 6.0);
        s.spacing.window_margin = egui::Margin::same(16);
    });
}
