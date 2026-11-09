use eframe::egui::{self, Color32, Rect};

pub fn raster(svg: &[u8], size: u32) -> Result<egui::ColorImage, String> {
    egui_extras::image::load_svg_bytes_with_size(
        svg,
        egui::load::SizeHint::Size {
            width: size,
            height: size,
            maintain_aspect_ratio: true,
        },
        &Default::default(),
    )
}
pub fn texture(
    ctx: &egui::Context,
    name: &'static str,
    svg: &[u8],
    size: u32,
) -> egui::TextureHandle {
    let key = egui::Id::new(("gavnel-svg", name, size));
    if let Some(texture) = ctx.data_mut(|data| data.get_temp::<egui::TextureHandle>(key)) {
        return texture;
    }
    let image = raster(svg, size).expect("Bundled SVG must be valid");
    let texture = ctx.load_texture(name, image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|data| data.insert_temp(key, texture.clone()));
    texture
}
pub fn paint(painter: &egui::Painter, name: &'static str, svg: &[u8], rect: Rect, tint: Color32) {
    let size = (rect.width().max(rect.height()) * painter.ctx().pixels_per_point() * 2.0)
        .ceil()
        .clamp(24.0, 256.0) as u32;
    let texture = texture(painter.ctx(), name, svg, size);
    painter.image(
        texture.id(),
        rect,
        Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
        tint,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_bundled_navigation_svg_renders() {
        let assets: [&[u8]; 17] = [
            include_bytes!("../assets/icons/hotkey-clip.svg"),
            include_bytes!("../assets/icons/hotkey-recording.svg"),
            include_bytes!("../assets/icons/hotkey-bookmark.svg"),
            include_bytes!("../assets/icons/hotkey-screenshot.svg"),
            include_bytes!("../assets/icons/hotkey-add.svg"),
            include_bytes!("../assets/icons/trash.svg"),
            include_bytes!("../assets/icons/settings.svg"),
            include_bytes!("../assets/icons/library.svg"),
            include_bytes!("../assets/icons/gamepad.svg"),
            include_bytes!("../assets/icons/capture.svg"),
            include_bytes!("../assets/icons/folder.svg"),
            include_bytes!("../assets/icons/search.svg"),
            include_bytes!("../assets/icons/microphone.svg"),
            include_bytes!("../assets/icons/volume.svg"),
            include_bytes!("../assets/icons/grid.svg"),
            include_bytes!("../assets/icons/play.svg"),
            include_bytes!("../assets/icons/refresh.svg"),
        ];
        for svg in assets {
            let image = raster(svg, 48).unwrap();
            assert_eq!(image.size, [48, 48]);
            assert!(image.pixels.iter().any(|pixel| pixel.a() > 0));
        }
    }
}
