//! Shared, test-only offscreen egui renderer.
use super::*;
use egui::Rect;

/// Logic-only tests intentionally do not upload textures to a GPU.
pub(super) fn run(
    ctx: &egui::Context,
    input: egui::RawInput,
    render: impl FnMut(&mut egui::Ui),
) -> egui::FullOutput {
    let mut output = ctx.run_ui(input, render);
    output.textures_delta.clear();
    output
}
pub(super) fn raster(
    ctx: &egui::Context,
    size: Vec2,
    textures: &std::collections::HashMap<egui::TextureId, egui::ColorImage>,
    output: egui::FullOutput,
) -> Vec<[u8; 3]> {
    let width = size.x as usize;
    let height = size.y as usize;
    let mut pixels = vec![[17, 17, 19]; width * height];
    let edge = |a: egui::Pos2, b: egui::Pos2, p: egui::Pos2| {
        (p.x - a.x) * (b.y - a.y) - (p.y - a.y) * (b.x - a.x)
    };
    for primitive in ctx.tessellate(output.shapes, output.pixels_per_point) {
        let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive else {
            continue;
        };
        let Some(texture) = textures.get(&mesh.texture_id) else {
            continue;
        };
        for indices in mesh.indices.as_chunks::<3>().0 {
            let vertices = indices.map(|i| mesh.vertices[i as usize]);
            let bounds = Rect::from_points(&vertices.map(|v| v.pos))
                .intersect(primitive.clip_rect)
                .intersect(Rect::from_min_size(egui::Pos2::ZERO, size));
            let area = edge(vertices[0].pos, vertices[1].pos, vertices[2].pos);
            if area.abs() < 0.00001 {
                continue;
            }
            for y in bounds.top().floor() as i32..bounds.bottom().ceil() as i32 {
                for x in bounds.left().floor() as i32..bounds.right().ceil() as i32 {
                    let p = egui::pos2(x as f32 + 0.5, y as f32 + 0.5);
                    let weights = [
                        edge(vertices[1].pos, vertices[2].pos, p) / area,
                        edge(vertices[2].pos, vertices[0].pos, p) / area,
                        edge(vertices[0].pos, vertices[1].pos, p) / area,
                    ];
                    if weights.iter().any(|v| *v < -0.00001) {
                        continue;
                    }
                    let uv = vertices
                        .iter()
                        .zip(weights)
                        .fold(Vec2::ZERO, |sum, (v, w)| sum + v.uv.to_vec2() * w);
                    let tx = (uv.x * texture.resolution()[0] as f32)
                        .floor()
                        .clamp(0.0, (texture.resolution()[0] - 1) as f32)
                        as usize;
                    let ty = (uv.y * texture.resolution()[1] as f32)
                        .floor()
                        .clamp(0.0, (texture.resolution()[1] - 1) as f32)
                        as usize;
                    let texel = texture.pixels()[ty * texture.resolution()[0] + tx].to_array();
                    let mut color = [0.0f32; 4];
                    for (v, w) in vertices.iter().zip(weights) {
                        for (ch, value) in v.color.to_array().iter().enumerate() {
                            color[ch] += *value as f32 / 255.0 * w;
                        }
                    }
                    let alpha = color[3] * texel[3] as f32 / 255.0;
                    let pixel = &mut pixels[y as usize * width + x as usize];
                    for ch in 0..3 {
                        pixel[ch] = (color[ch] * texel[ch] as f32
                            + pixel[ch] as f32 * (1.0 - alpha))
                            .clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }
    }
    pixels
}
pub(super) fn write_bitmap(path: &str, width: usize, height: usize, sheet: &[[u8; 3]]) {
    use std::io::Write;
    let mut file = std::fs::File::create(path).unwrap();
    let stride = (width * 3).div_ceil(4) * 4;
    file.write_all(b"BM").unwrap();
    file.write_all(&((54 + stride * height) as u32).to_le_bytes())
        .unwrap();
    file.write_all(&[0; 4]).unwrap();
    file.write_all(&54u32.to_le_bytes()).unwrap();
    file.write_all(&40u32.to_le_bytes()).unwrap();
    file.write_all(&(width as i32).to_le_bytes()).unwrap();
    file.write_all(&(height as i32).to_le_bytes()).unwrap();
    file.write_all(&1u16.to_le_bytes()).unwrap();
    file.write_all(&24u16.to_le_bytes()).unwrap();
    file.write_all(&[0; 24]).unwrap();
    for y in (0..height).rev() {
        for x in 0..width {
            let p = sheet[y * width + x];
            file.write_all(&[p[2], p[1], p[0]]).unwrap();
        }
        file.write_all(&vec![0; stride - width * 3]).unwrap();
    }
}
