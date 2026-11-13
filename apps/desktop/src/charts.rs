//! Small charts drawn with the painter (no plotting crate): bars, columns, a stacked bar and the walk from the entrance to the exit.

use eframe::egui::{
    Align2, Color32, FontId, Grid, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2,
};

pub struct Bar {
    pub label: String,
    pub value: f64,
    pub color: Color32,
}

impl Bar {
    pub fn new(label: impl Into<String>, value: f64, color: Color32) -> Self {
        Self {
            label: label.into(),
            value,
            color,
        }
    }
}

fn number(value: f64) -> String {
    if value.fract().abs() < 1e-9 {
        format!("{}", value as i64)
    } else {
        format!("{value:.1}")
    }
}

/// One row per bar: the label on the left, the bar on a track, the value at the end. `max` is the length of a full track.
pub fn hbars(ui: &mut Ui, bars: &[Bar], max: f64) {
    let row = 24.0;
    let size = Vec2::new(ui.available_width(), row * bars.len() as f32);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);
    let text = ui.visuals().text_color();
    let track = ui.visuals().extreme_bg_color;
    let font = FontId::proportional(13.0);
    let label_w = (rect.width() * 0.34).clamp(80.0, 150.0);
    let value_w = 40.0;
    for (i, bar) in bars.iter().enumerate() {
        let y = rect.top() + row * i as f32;
        let mid = y + row / 2.0;
        painter.text(
            Pos2::new(rect.left(), mid),
            Align2::LEFT_CENTER,
            &bar.label,
            font.clone(),
            text,
        );
        let full = Rect::from_min_max(
            Pos2::new(rect.left() + label_w, y + 5.0),
            Pos2::new(rect.right() - value_w, y + row - 5.0),
        );
        painter.rect_filled(full, 4.0, track);
        if bar.value > 0.0 && max > 0.0 {
            let w = ((bar.value / max) as f32).clamp(0.0, 1.0) * full.width();
            let filled = Rect::from_min_size(full.min, Vec2::new(w.max(3.0), full.height()));
            painter.rect_filled(filled, 4.0, bar.color);
        }
        painter.text(
            Pos2::new(rect.right(), mid),
            Align2::RIGHT_CENTER,
            number(bar.value),
            font.clone(),
            text,
        );
    }
}

/// Vertical bars side by side, each with its value above and its label below.
pub fn columns(ui: &mut Ui, bars: &[Bar], max: f64, height: f32) {
    let size = Vec2::new(ui.available_width(), height);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);
    let text = ui.visuals().text_color();
    let track = ui.visuals().extreme_bg_color;
    let font = FontId::proportional(13.0);
    let n = bars.len().max(1) as f32;
    let slot = rect.width() / n;
    let top = rect.top() + 18.0;
    let bottom = rect.bottom() - 20.0;
    for (i, bar) in bars.iter().enumerate() {
        let x0 = rect.left() + slot * i as f32 + slot * 0.18;
        let x1 = rect.left() + slot * (i + 1) as f32 - slot * 0.18;
        let full = Rect::from_min_max(Pos2::new(x0, top), Pos2::new(x1, bottom));
        painter.rect_filled(full, 4.0, track);
        let h = if max > 0.0 {
            ((bar.value / max) as f32).clamp(0.0, 1.0) * full.height()
        } else {
            0.0
        };
        if bar.value > 0.0 {
            let filled = Rect::from_min_max(
                Pos2::new(full.left(), full.bottom() - h.max(3.0)),
                full.right_bottom(),
            );
            painter.rect_filled(filled, 4.0, bar.color);
        }
        painter.text(
            Pos2::new((x0 + x1) / 2.0, top - 3.0),
            Align2::CENTER_BOTTOM,
            number(bar.value),
            font.clone(),
            text,
        );
        painter.text(
            Pos2::new((x0 + x1) / 2.0, bottom + 4.0),
            Align2::CENTER_TOP,
            &bar.label,
            font.clone(),
            text,
        );
    }
}

/// One bar cut in parts in proportion to their values, with a legend below.
pub fn stacked(ui: &mut Ui, parts: &[Bar]) {
    let total: f64 = parts.iter().map(|p| p.value.max(0.0)).sum();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 5.0, ui.visuals().extreme_bg_color);
    if total > 0.0 {
        let mut x = rect.left();
        for part in parts {
            let w = (part.value.max(0.0) / total) as f32 * rect.width();
            if w > 0.0 {
                let slice =
                    Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(w, rect.height()));
                painter.rect_filled(slice.shrink2(Vec2::new(0.0, 0.0)), 0.0, part.color);
                x += w;
            }
        }
        painter.rect_stroke(
            rect,
            5.0,
            Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
            StrokeKind::Inside,
        );
    }
    ui.add_space(4.0);
    Grid::new("stacked-legend")
        .num_columns(2)
        .spacing([16.0, 4.0])
        .show(ui, |ui| {
            for (i, part) in parts.iter().enumerate() {
                ui.horizontal(|ui| {
                    let (dot, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                    ui.painter().rect_filled(dot, 3.0, part.color);
                    let share = if total > 0.0 {
                        part.value / total * 100.0
                    } else {
                        0.0
                    };
                    ui.label(format!("{} · {share:.0}%", part.label));
                });
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
}

/// The walk from the entrance to the exit: one dot per room crossed, the entrance in `from`, the exit in `to`.
pub fn walk(ui: &mut Ui, steps: usize, from: Color32, to: Color32, line: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let dots = steps.clamp(1, 24) + 1;
    let (x0, x1) = (rect.left() + 10.0, rect.right() - 10.0);
    let y = rect.center().y;
    painter.line_segment([Pos2::new(x0, y), Pos2::new(x1, y)], Stroke::new(3.0, line));
    for i in 0..dots {
        let t = if dots > 1 {
            i as f32 / (dots - 1) as f32
        } else {
            0.0
        };
        let color = if i == 0 {
            from
        } else if i + 1 == dots {
            to
        } else {
            line
        };
        let radius = if i == 0 || i + 1 == dots { 7.0 } else { 4.5 };
        painter.circle_filled(Pos2::new(x0 + (x1 - x0) * t, y), radius, color);
    }
    if steps > 24 {
        painter.text(
            Pos2::new(rect.center().x, rect.top()),
            Align2::CENTER_TOP,
            format!("{steps} rooms in all"),
            FontId::proportional(11.0),
            line,
        );
    }
}
