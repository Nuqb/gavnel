use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
use std::{io::Write, path::Path};

const ICON: &[u8] = include_bytes!("../assets/gavnel-icon.svg");
const BACKGROUND: &[u8] = include_bytes!("../assets/icon-background.svg");
const MARK: &[u8] = include_bytes!("../assets/bullet-mark.svg");

pub fn explosion(phase: f32) -> f32 {
    if !(0.12..=0.88).contains(&phase) {
        return 0.0;
    }
    (((phase - 0.12) / 0.76) * std::f32::consts::PI)
        .sin()
        .powi(2)
}
pub fn paint(painter: &egui::Painter, rect: Rect, phase: Option<f32>) {
    let rect = Rect::from_center_size(rect.center(), Vec2::splat(rect.width().min(rect.height())));
    let amount = phase.map(explosion).unwrap_or(0.0);
    if amount < 0.001 {
        crate::icons::paint(painter, "app-icon", ICON, rect, Color32::WHITE);
        return;
    }
    crate::icons::paint(
        painter,
        "app-icon-background",
        BACKGROUND,
        rect,
        Color32::WHITE,
    );
    let texture = crate::icons::texture(painter.ctx(), "bullet-mark", MARK, 128);
    let mut mesh = egui::Mesh::with_texture(texture.id());
    // Triangular UV fragments cut the actual SVG artwork into shrapnel. At
    // both ends all vertices return exactly to the unbroken SVG silhouette.
    for row in 0..3 {
        for column in 0..3 {
            for half in 0..2 {
                let x = column as f32 / 3.0;
                let y = row as f32 / 3.0;
                let step = 1.0 / 3.0;
                let points = if half == 0 {
                    [
                        Vec2::new(x, y),
                        Vec2::new(x + step, y),
                        Vec2::new(x, y + step),
                    ]
                } else {
                    [
                        Vec2::new(x + step, y),
                        Vec2::new(x + step, y + step),
                        Vec2::new(x, y + step),
                    ]
                };
                let center = (points[0] + points[1] + points[2]) / 3.0;
                let direction = (center - Vec2::splat(0.5)).normalized();
                let displacement = direction * amount * 0.24;
                let angle = amount
                    * if (row + column + half) % 2 == 0 {
                        -0.65
                    } else {
                        0.65
                    };
                let (sin, cos) = angle.sin_cos();
                let start = mesh.vertices.len() as u32;
                for uv in points {
                    let local = uv - center;
                    let local =
                        Vec2::new(local.x * cos - local.y * sin, local.x * sin + local.y * cos);
                    let position = center + local + displacement;
                    mesh.vertices.push(egui::epaint::Vertex {
                        pos: rect.min + position * rect.width(),
                        uv: Pos2::new(uv.x, uv.y),
                        color: Color32::WHITE,
                    });
                }
                mesh.indices
                    .extend_from_slice(&[start, start + 1, start + 2]);
            }
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}
pub fn icon_rgba(size: usize) -> Vec<u8> {
    crate::icons::raster(ICON, size as u32)
        .expect("Bundled app icon must render")
        .pixels
        .iter()
        .flat_map(|p| p.to_srgba_unmultiplied())
        .collect()
}

pub fn export_windows_icon(folder: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(folder)?;
    let sizes = [16usize, 24, 32, 48, 64, 128, 256];
    let mut images = Vec::new();
    for size in sizes {
        let rgba = icon_rgba(size);
        let mask_stride = size.div_ceil(32) * 4;
        let mut image = Vec::new();
        for number in [40u32, size as u32, (size * 2) as u32] {
            image.extend_from_slice(&number.to_le_bytes());
        }
        image.extend_from_slice(&1u16.to_le_bytes());
        image.extend_from_slice(&32u16.to_le_bytes());
        for number in [
            0u32,
            (size * size * 4 + mask_stride * size) as u32,
            0,
            0,
            0,
            0,
        ] {
            image.extend_from_slice(&number.to_le_bytes());
        }
        for y in (0..size).rev() {
            for x in 0..size {
                let p = &rgba[(y * size + x) * 4..][..4];
                image.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
            }
        }
        for y in (0..size).rev() {
            let mut mask = vec![0u8; mask_stride];
            for x in 0..size {
                if rgba[(y * size + x) * 4 + 3] == 0 {
                    mask[x / 8] |= 0x80 >> (x % 8);
                }
            }
            image.extend_from_slice(&mask);
        }
        images.push(image);
    }
    let mut file = std::fs::File::create(folder.join("gavnel.ico"))?;
    file.write_all(&[0, 0, 1, 0, sizes.len() as u8, 0])?;
    let mut offset = 6 + 16 * sizes.len();
    for (size, image) in sizes.iter().zip(&images) {
        let dimension = if *size == 256 { 0 } else { *size as u8 };
        file.write_all(&[dimension, dimension, 0, 0])?;
        file.write_all(&1u16.to_le_bytes())?;
        file.write_all(&32u16.to_le_bytes())?;
        file.write_all(&(image.len() as u32).to_le_bytes())?;
        file.write_all(&(offset as u32).to_le_bytes())?;
        offset += image.len();
    }
    for image in images {
        file.write_all(&image)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fragments_reassemble_at_both_ends() {
        assert_eq!(explosion(0.0), 0.0);
        assert_eq!(explosion(1.0), 0.0);
        assert!(explosion(0.5) > 0.99);
    }
    #[test]
    fn minimal_icon_has_rounded_gray_tile_and_transparent_corners() {
        let pixels = icon_rgba(32);
        assert_eq!(pixels.len(), 4096);
        assert_eq!(&pixels[..4], &[0, 0, 0, 0]);
        let background = &pixels[(16 * 32 + 3) * 4..][..4];
        assert_eq!(background, &[37, 37, 41, 255]);
        assert!(
            pixels
                .as_chunks::<4>()
                .0
                .iter()
                .any(|p| p[0] > 240 && p[1] > 100 && p[2] < 150)
        );
    }
    #[test]
    fn desktop_icon_contains_all_windows_sizes() {
        let folder = tempfile::tempdir().unwrap();
        export_windows_icon(folder.path()).unwrap();
        let bytes = std::fs::read(folder.path().join("gavnel.ico")).unwrap();
        assert_eq!(&bytes[..6], &[0, 0, 1, 0, 7, 0]);
        assert_eq!(bytes[6], 16);
        assert_eq!(bytes[6 + 16 * 6], 0);
    }
}
