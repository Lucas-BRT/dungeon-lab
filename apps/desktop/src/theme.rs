//! The look of the app, taken from the UI of the Zed editor (zed-industries/zed, `main`): its One Dark and One Light colours
//! (`assets/themes/one/one.json`), its 4 px spacing grid (`crates/ui/src/styles/spacing.rs`: gaps of 4, 6 and 8 px), its type sizes
//! (`typography.rs`: 12, 14 and 16 px for text, 18 px for a headline), a tab bar 32 px high with 1 px borders (`tab.rs`, `tab_bar.rs`),
//! buttons 28 px (medium) and 22 px (default) high with 8 px or 4 px of padding (`button_like.rs`), and surfaces that rise from the
//! window background to the panel, the element and the editor (`elevation.rs`). The radius of a button (`rounded_sm`) was not in the
//! files that were read; 4 px is a guess.

use eframe::egui::{self, Color32, CornerRadius, Margin, Stroke, Theme};

/// Zed's `Base04`, `Base06` and `Base08` spacings, in points.
pub const GAP: f32 = 4.0;
pub const GAP_MEDIUM: f32 = 6.0;
pub const PAD: f32 = 8.0;
/// A tab and the bar of tabs are 32 px high in Zed.
pub const TAB_HEIGHT: f32 = 32.0;
pub const RADIUS: u8 = 4;

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

#[derive(Clone, Copy)]
pub struct Colors {
    /// Title bar and status bar (`title_bar.background`, `status_bar.background`).
    pub bar: Color32,
    /// Panels and the tab bar (`panel.background`, `tab_bar.background`).
    pub panel: Color32,
    /// The editor, here the map (`editor.background`, `tab.active_background`).
    pub editor: Color32,
    pub element: Color32,
    pub element_hover: Color32,
    pub element_active: Color32,
    pub border: Color32,
    pub border_variant: Color32,
    pub border_focused: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub accent: Color32,
    pub success: Color32,
    pub error: Color32,
}

pub fn colors(dark: bool) -> Colors {
    if dark {
        Colors {
            bar: rgb(0x3b414d),
            panel: rgb(0x2f343e),
            editor: rgb(0x282c33),
            element: rgb(0x2e343e),
            element_hover: rgb(0x363c46),
            element_active: rgb(0x454a56),
            border: rgb(0x464b57),
            border_variant: rgb(0x363c46),
            border_focused: rgb(0x47679e),
            text: rgb(0xdce0e5),
            text_muted: rgb(0xa9afbc),
            accent: rgb(0x74ade8),
            success: rgb(0xa1c181),
            error: rgb(0xd07277),
        }
    } else {
        Colors {
            bar: rgb(0xdcdcdd),
            panel: rgb(0xebebec),
            editor: rgb(0xfafafa),
            element: rgb(0xebebec),
            element_hover: rgb(0xdfdfe0),
            element_active: rgb(0xcacaca),
            border: rgb(0xc9c9ca),
            border_variant: rgb(0xdfdfe0),
            border_focused: rgb(0x7d82e8),
            text: rgb(0x242529),
            text_muted: rgb(0x58585a),
            accent: rgb(0x5c78e2),
            success: rgb(0x669f59),
            error: rgb(0xd36151),
        }
    }
}

pub fn of(ui: &egui::Ui) -> Colors {
    colors(ui.visuals().dark_mode)
}

/// A bar (title or status): the window background of the theme, a 1 px border on the side facing the content.
pub fn bar_frame(ui: &egui::Ui) -> egui::Frame {
    let c = of(ui);
    egui::Frame::new()
        .fill(c.bar)
        .inner_margin(Margin::symmetric(PAD as i8, GAP as i8))
}

/// A panel: the panel surface.
pub fn panel_frame(ui: &egui::Ui) -> egui::Frame {
    let c = of(ui);
    egui::Frame::new()
        .fill(c.panel)
        .inner_margin(Margin::symmetric(PAD as i8, GAP as i8))
}

/// A card: an element on a panel, with a 1 px border and the radius of the controls, a little larger.
pub fn card_frame(ui: &egui::Ui) -> egui::Frame {
    let c = of(ui);
    egui::Frame::new()
        .fill(c.element)
        .stroke(Stroke::new(1.0, c.border_variant))
        .corner_radius(CornerRadius::same(RADIUS + 2))
        .inner_margin(Margin::same(PAD as i8 + 2))
}

/// The text and surfaces of every widget, for both themes, in Zed's tokens.
pub fn apply(ctx: &egui::Context) {
    for (theme, dark) in [(Theme::Dark, true), (Theme::Light, false)] {
        let c = colors(dark);
        ctx.style_mut_of(theme, |style| {
            let radius = CornerRadius::same(RADIUS);
            let v = &mut style.visuals;
            v.panel_fill = c.panel;
            v.window_fill = c.panel;
            v.window_stroke = Stroke::new(1.0, c.border);
            v.window_corner_radius = CornerRadius::same(RADIUS + 4);
            v.extreme_bg_color = c.editor;
            v.faint_bg_color = c.element_hover;
            v.code_bg_color = c.editor;
            v.hyperlink_color = c.accent;
            v.weak_text_color = Some(c.text_muted);
            v.selection.bg_fill = c.element_active;
            v.selection.stroke = Stroke::new(1.0, c.accent);
            v.widgets.noninteractive.bg_fill = c.panel;
            v.widgets.noninteractive.weak_bg_fill = c.panel;
            v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, c.border_variant);
            v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, c.text);
            for (widget, fill) in [
                (&mut v.widgets.inactive, c.element),
                (&mut v.widgets.hovered, c.element_hover),
                (&mut v.widgets.active, c.element_active),
                (&mut v.widgets.open, c.element_hover),
            ] {
                widget.bg_fill = fill;
                widget.weak_bg_fill = fill;
                widget.bg_stroke = Stroke::new(1.0, c.border_variant);
                widget.fg_stroke = Stroke::new(1.0, c.text);
                widget.corner_radius = radius;
                widget.expansion = 0.0;
            }
            // Inputs (drag values, check boxes, text boxes) sit on the editor surface; buttons on the element surface.
            v.widgets.inactive.bg_fill = c.editor;
            v.widgets.hovered.bg_fill = c.editor;
            v.widgets.noninteractive.corner_radius = radius;
            v.widgets.hovered.bg_stroke = Stroke::new(1.0, c.border);
            v.widgets.active.bg_stroke = Stroke::new(1.0, c.border_focused);

            let s = &mut style.spacing;
            s.item_spacing = egui::vec2(PAD, GAP_MEDIUM);
            s.button_padding = egui::vec2(PAD, GAP);
            s.interact_size.y = 28.0;
            s.indent = 12.0;
            s.window_margin = Margin::same(PAD as i8);
            s.menu_margin = Margin::same(GAP as i8);

            style
                .text_styles
                .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
            style
                .text_styles
                .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
            style
                .text_styles
                .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
            style
                .text_styles
                .insert(egui::TextStyle::Heading, egui::FontId::proportional(18.0));
            style
                .text_styles
                .insert(egui::TextStyle::Monospace, egui::FontId::monospace(13.0));
        });
    }
}

/// One tab of a tab bar, as Zed draws it: 32 px high, the active one on the editor surface with the accent along its top edge, the
/// others on the bar in muted text, a 1 px border under all of them. Returns the response for clicks.
pub fn tab(ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
    let c = of(ui);
    let font = egui::TextStyle::Body.resolve(ui.style());
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        font,
        if selected { c.text } else { c.text_muted },
    );
    let size = egui::vec2(galley.size().x + 2.0 * PAD, TAB_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let fill = if selected {
        c.editor
    } else if response.hovered() {
        c.element_hover
    } else {
        c.panel
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, fill);
    painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0, c.border_variant),
    );
    if selected {
        painter.line_segment(
            [rect.left_top(), rect.right_top()],
            Stroke::new(2.0, c.accent),
        );
    }
    let color = if selected {
        c.text
    } else if response.hovered() {
        c.text
    } else {
        c.text_muted
    };
    painter.galley(
        egui::pos2(rect.left() + PAD, rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
    response
}
