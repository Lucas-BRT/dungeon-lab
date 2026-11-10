use crate::i18n;
use crate::json_value_text;
use eframe::egui::{self, DragValue, RichText, Ui};
use serde_json::{json, Value};
pub use tapiti_schema::{groups, FieldKind, ParamField, ParamGroup};

/// One parameter as a grid row: label with hover help, the control, and a reset when it differs from the level default.
/// Returns the new value when the user changed it.
pub fn field_row(
    ui: &mut Ui,
    field: &ParamField,
    current: &Value,
    default: Option<&Value>,
) -> Option<Value> {
    let differs = default.is_some_and(|d| d != current);
    let label = RichText::new(field.label.get(i18n::is_pt()));
    let label = if differs {
        label.color(ui.visuals().selection.bg_fill).strong()
    } else {
        label
    };
    ui.label(label).on_hover_text(field.help.get(i18n::is_pt()));
    let mut next = None;
    match &field.kind {
        FieldKind::Number { min, max, integer } => {
            let mut v = current.as_f64().unwrap_or(0.0);
            let speed = if *integer {
                0.2
            } else {
                (v.abs() * 0.02).max((max - min) / 2000.0).clamp(0.001, 5.0)
            };
            let mut drag = DragValue::new(&mut v)
                .range(*min..=*max)
                .speed(speed)
                .update_while_editing(false);
            drag = if *integer {
                drag.fixed_decimals(0)
            } else {
                drag.max_decimals(3)
            };
            if ui
                .add(drag)
                .on_hover_text(field.help.get(i18n::is_pt()))
                .changed()
            {
                next = Some(if *integer {
                    json!(v.round() as i64)
                } else {
                    json!(v)
                });
            }
        }
        FieldKind::Boolean => {
            let mut b = current.as_bool().unwrap_or(false);
            if ui
                .checkbox(&mut b, "")
                .on_hover_text(field.help.get(i18n::is_pt()))
                .changed()
            {
                next = Some(json!(b));
            }
        }
        FieldKind::Choice(options) => {
            let text = json_value_text(current);
            let shown = options
                .iter()
                .find(|(v, _)| *v == text)
                .map_or(text.as_str(), |(_, n)| n.get(i18n::is_pt()));
            egui::ComboBox::from_id_salt(&field.path)
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for (value, name) in options {
                        if ui
                            .selectable_label(*value == text, name.get(i18n::is_pt()))
                            .clicked()
                            && *value != text
                        {
                            next = Some(
                                if matches!(field.path.as_str(), "m" | "discPorts" | "maxBends") {
                                    value.parse::<i64>().map(Value::from).unwrap_or(Value::Null)
                                } else {
                                    Value::String(value.clone())
                                },
                            );
                        }
                    }
                });
        }
    }
    if differs {
        if ui
            .small_button(t!("default"))
            .on_hover_text(t!("Reset to the level default"))
            .clicked()
        {
            next = default.cloned();
        }
    } else {
        ui.label("");
    }
    ui.end_row();
    next
}
