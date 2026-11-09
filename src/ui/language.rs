//! Searchable native-name picker; incomplete drafts stay unavailable.
use super::*;

fn matches(language: i18n::Language, query: &str) -> bool {
    language.is_selectable()
        && (query.is_empty()
            || language.name().to_lowercase().contains(query)
            || language.english_name().to_lowercase().contains(query)
            || language.code().to_lowercase().contains(query))
}

#[derive(Clone, Default)]
struct Picker {
    open: bool,
    query: String,
}

pub(super) fn selector(ui: &mut egui::Ui, selected: &mut i18n::Language, id: &str) {
    let id = ui.make_persistent_id(id);
    let mut picker = ui
        .data(|data| data.get_temp::<Picker>(id))
        .unwrap_or_default();
    let button = ui.add_sized([270.0, 38.0], egui::Button::new(selected.name()));
    let center = egui::pos2(button.rect.right() - 17.0, button.rect.center().y);
    ui.painter().add(egui::Shape::line(
        vec![
            center + egui::vec2(-4.0, -2.0),
            center + egui::vec2(0.0, 2.0),
            center + egui::vec2(4.0, -2.0),
        ],
        egui::Stroke::new(1.5, MUTED),
    ));
    let opened = button.clicked();
    if opened {
        picker.open = true;
        picker.query.clear();
    }
    if picker.open {
        show(ui.ctx(), id, selected, &mut picker, opened);
    }
    ui.data_mut(|data| data.insert_temp(id, picker));
}

fn show(
    ctx: &egui::Context,
    id: egui::Id,
    selected: &mut i18n::Language,
    picker: &mut Picker,
    focus: bool,
) {
    let response = egui::Modal::new(id.with("modal"))
        .backdrop_color(Color32::from_black_alpha(180))
        .frame(
            egui::Frame::new()
                .fill(PANEL)
                .stroke(egui::Stroke::new(1.0, LINE))
                .corner_radius(16.0)
                .inner_margin(24.0),
        )
        .show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 96.0).clamp(240.0, 680.0));
            ui.horizontal(|ui| {
                ui.heading(tr("Language"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("×").on_hover_text(tr("Close")).clicked() {
                        picker.open = false;
                    }
                });
            });
            ui.add_space(8.0);
            let search = ui.add(
                egui::TextEdit::singleline(&mut picker.query)
                    .id(id.with("search"))
                    .hint_text(tr("Language"))
                    .desired_width(f32::INFINITY),
            );
            if focus {
                search.request_focus();
            }
            ui.add_space(8.0);
            let query = picker.query.trim().to_lowercase();
            let height = (ctx.content_rect().height() - 230.0).clamp(160.0, 360.0);
            egui::ScrollArea::vertical()
                .id_salt(id.with("results"))
                .max_height(height)
                .min_scrolled_height(height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if let Some(language) = choices(ui, *selected, &query, focus) {
                        *selected = language;
                        picker.open = false;
                    }
                });
        });
    if response.should_close() {
        picker.open = false;
    }
}

fn choices(
    ui: &mut egui::Ui,
    selected: i18n::Language,
    query: &str,
    focus: bool,
) -> Option<i18n::Language> {
    let languages: Vec<_> = i18n::Language::ALL
        .into_iter()
        .filter(|language| matches(*language, query))
        .collect();
    if languages.is_empty() {
        ui.label(RichText::new(tr("None")).color(MUTED));
        return None;
    }
    let mut picked = None;
    for pair in languages.chunks(2) {
        ui.columns(2, |columns| {
            for (column, &language) in columns.iter_mut().zip(pair) {
                let response = row(column, language, selected == language);
                if focus && selected == language {
                    response.scroll_to_me(Some(egui::Align::Center));
                }
                if response.clicked() {
                    picked = Some(language);
                }
            }
        });
    }
    picked
}

fn row(ui: &mut egui::Ui, language: i18n::Language, selected: bool) -> egui::Response {
    let response = egui::Frame::new()
        .fill(if selected { ACCENT_DARK } else { CARD })
        .corner_radius(8.0)
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width((ui.available_width() - 24.0).max(100.0));
                    ui.label(RichText::new(language.name()).color(if selected {
                        ACCENT
                    } else {
                        TEXT
                    }));
                    ui.label(
                        RichText::new(format!("{} · {}", language.english_name(), language.code()))
                            .size(11.0)
                            .color(MUTED),
                    );
                });
                if selected {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.colored_label(ACCENT, "✓");
                    });
                }
            });
        })
        .response
        .interact(egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, true, selected, language.name())
    });
    if response.hovered() {
        ui.painter().rect_stroke(
            response.rect,
            8.0,
            egui::Stroke::new(1.0, ACCENT),
            egui::StrokeKind::Inside,
        );
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_accepts_native_names_english_names_and_region_tags_but_hides_drafts() {
        assert!(matches(i18n::Language::Hindi, "hindi"));
        assert!(matches(i18n::Language::Hindi, "हिन्दी"));
        assert!(matches(i18n::Language::SpanishMexico, "es-mx"));
        assert!(!matches(i18n::Language::Yoruba, ""));
    }
}
