//! Live capture feedback card; clicking it lets the caller choose a source.
use super::views::{Glyph, paint_icon};
use super::*;
use egui::{Align2, FontId, Rect, Sense};
pub(super) fn show(ui: &mut egui::Ui, status: &Status) -> egui::Response {
    let preview = status.preview.as_ref().filter(|_| status.recording);
    let height = if preview.is_some() { 219.0 } else { 106.0 };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(332.0, height), Sense::click());
    ui.painter()
        .rect_filled(rect, 9.0, Color32::from_rgb(42, 42, 46));
    if let Some(image) = preview {
        let key = egui::Id::unique("recording-source-preview");
        let mut cached = ui
            .ctx()
            .data_mut(|data| data.get_temp::<(Arc<egui::ColorImage>, egui::TextureHandle)>(key));
        if cached
            .as_ref()
            .is_none_or(|(previous, _)| !Arc::ptr_eq(previous, image))
        {
            let texture = ui.ctx().load_texture(
                "recording-source-preview",
                image.as_ref().clone(),
                egui::TextureOptions::LINEAR,
            );
            cached = Some((image.clone(), texture));
        }
        let (_, texture) = cached.as_ref().unwrap();
        let area = Rect::from_min_size(rect.min, Vec2::new(332.0, 187.0));
        let size = texture.resolution_vec2();
        let scale = (area.width() / size.x).min(area.height() / size.y);
        let image_rect = Rect::from_center_size(area.center(), size * scale);
        ui.painter().image(
            texture.id(),
            image_rect,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        ui.painter().text(
            egui::pos2(rect.center().x, rect.bottom() - 16.0),
            Align2::CENTER_CENTER,
            format!("●  {}", status.source.chars().take(35).collect::<String>()),
            FontId::proportional(12.0),
            TEXT,
        );
        ui.ctx()
            .data_mut(|data| data.insert_temp(key, cached.unwrap()));
    } else {
        paint_icon(
            ui.painter(),
            Rect::from_center_size(
                egui::pos2(rect.center().x, rect.top() + 28.0),
                Vec2::splat(22.0),
            ),
            Glyph::Game,
            MUTED,
        );
        let title = if status.recording {
            status.source.chars().take(35).collect::<String>()
        } else if status.armed {
            tr("Waiting for a game to be detected").into()
        } else {
            tr("Ready for your next play").into()
        };
        ui.painter().text(
            egui::pos2(rect.center().x, rect.top() + 58.0),
            Align2::CENTER_CENTER,
            title,
            FontId::new(14.0, egui::FontFamily::Name("GavnelSemibold".into())),
            TEXT,
        );
        ui.painter().text(
            egui::pos2(rect.center().x, rect.top() + 82.0),
            Align2::CENTER_CENTER,
            if status.recording {
                tr("Preview unavailable · click to change source")
            } else {
                tr("Choose a game, monitor, or window")
            },
            FontId::proportional(12.0),
            MUTED,
        );
    }
    response
}
