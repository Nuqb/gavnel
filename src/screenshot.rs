use anyhow::{Context, Result};
use std::{
    fs::OpenOptions,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};
use windows::Win32::Graphics::{Direct3D11::*, Dxgi::Common::*};

pub struct Screenshot {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Screenshot {
    pub fn preview(&self) -> eframe::egui::ColorImage {
        let scale = (332.0 / self.width as f64)
            .min(187.0 / self.height as f64)
            .min(1.0);
        let width = (self.width as f64 * scale).round().max(1.0) as usize;
        let height = (self.height as f64 * scale).round().max(1.0) as usize;
        let mut rgba = Vec::with_capacity(width * height * 4);
        for y in 0..height {
            for x in 0..width {
                let offset = ((y * self.height as usize / height) * self.width as usize
                    + x * self.width as usize / width)
                    * 4;
                rgba.extend_from_slice(&self.rgba[offset..offset + 4]);
            }
        }
        eframe::egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba)
    }
}

pub fn read_texture(
    device: &ID3D11Device,
    context: &ID3D11DeviceContext,
    texture: &ID3D11Texture2D,
) -> Result<Screenshot> {
    unsafe {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        texture.GetDesc(&mut desc);
        anyhow::ensure!(
            matches!(
                desc.Format,
                DXGI_FORMAT_B8G8R8A8_UNORM | DXGI_FORMAT_B8G8R8A8_UNORM_SRGB
            ),
            "Screenshot pixel format is unavailable."
        );
        let size = (desc.Width as usize)
            .checked_mul(desc.Height as usize)
            .and_then(|n| n.checked_mul(4))
            .context("Screenshot is too large")?;
        anyhow::ensure!(
            size > 0 && size <= 256 * 1024 * 1024,
            "Screenshot is too large"
        );
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.MiscFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        let mut staging = None;
        device.CreateTexture2D(&desc, None, Some(&mut staging))?;
        let staging = staging.context("Could not prepare screenshot")?;
        context.CopyResource(&staging, texture);
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
        let result = (|| -> Result<_> {
            anyhow::ensure!(
                !mapped.pData.is_null() && mapped.RowPitch as usize >= desc.Width as usize * 4,
                "Could not read screenshot pixels"
            );
            let mut rgba = vec![0; size];
            for y in 0..desc.Height as usize {
                let row = std::slice::from_raw_parts(
                    mapped.pData.cast::<u8>().add(y * mapped.RowPitch as usize),
                    desc.Width as usize * 4,
                );
                for (input, output) in row.as_chunks::<4>().0.iter().zip(
                    rgba[y * desc.Width as usize * 4..(y + 1) * desc.Width as usize * 4]
                        .as_chunks_mut::<4>()
                        .0,
                ) {
                    *output = [input[2], input[1], input[0], 255];
                }
            }
            Ok(Screenshot {
                width: desc.Width,
                height: desc.Height,
                rgba,
            })
        })();
        context.Unmap(&staging, 0);
        result
    }
}

pub fn save(image: &Screenshot, root: &Path, source: &str, limit: Option<u32>) -> Result<PathBuf> {
    anyhow::ensure!(
        image.width > 0
            && image.height > 0
            && image.rgba.len() as u64 == u64::from(image.width) * u64::from(image.height) * 4,
        "Invalid screenshot pixels"
    );
    crate::storage::check_limit(root, limit, image.rgba.len() as u64)?;
    let folder = crate::export::source_folder(root, source)?;
    let base = format!(
        "{}_{}_screenshot",
        crate::export::safe_name(source),
        chrono::Local::now().format("%Y-%m-%d_%H-%M-%S_%3f")
    );
    for suffix in 0..1000 {
        let name = if suffix == 0 {
            base.clone()
        } else {
            format!("{base}_{suffix}")
        };
        let path = folder.join(format!("{name}.png"));
        if path.exists() {
            continue;
        }
        let temporary = folder.join(format!("{name}.png.partial"));
        let file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        };
        let result = (|| -> Result<()> {
            let mut file = BufWriter::new(file);
            let mut encoder = png::Encoder::new(&mut file, image.width, image.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header()?;
            writer.write_image_data(&image.rgba)?;
            writer.finish()?;
            file.flush()?;
            file.get_ref().sync_all()?;
            drop(file);
            std::fs::rename(&temporary, &path)?;
            Ok(())
        })();
        if let Err(error) = result {
            let _ = std::fs::remove_file(&temporary);
            return Err(error);
        }
        return Ok(path);
    }
    anyhow::bail!("Could not choose a screenshot filename")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_fits_card_and_preserves_aspect_and_colors() {
        for (width, height, expected) in [
            (1920, 1080, [332, 187]),
            (1080, 1920, [105, 187]),
            (2, 2, [2, 2]),
        ] {
            let image = Screenshot {
                width,
                height,
                rgba: [12, 34, 56, 255].repeat((width * height) as usize),
            };
            let preview = image.preview();
            assert_eq!(preview.size, expected);
            assert!(
                preview
                    .pixels
                    .iter()
                    .all(|pixel| *pixel == eframe::egui::Color32::from_rgb(12, 34, 56))
            );
        }
    }
    #[test]
    #[ignore = "requires Direct3D; captures only generated test pixels"]
    fn gpu_readback_preserves_colors_dimensions_and_padded_row_pitch() {
        let (device, context) = windows_capture::d3d11::create_d3d_device().unwrap();
        let bgra: [u8; 16] = [
            0, 0, 255, 255, 255, 0, 0, 255, 0, 255, 0, 255, 255, 255, 255, 255,
        ];
        let desc = D3D11_TEXTURE2D_DESC {
            Width: 2,
            Height: 2,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            ..Default::default()
        };
        let data = D3D11_SUBRESOURCE_DATA {
            pSysMem: bgra.as_ptr().cast(),
            SysMemPitch: 8,
            SysMemSlicePitch: 16,
        };
        let mut texture = None;
        unsafe {
            device
                .CreateTexture2D(&desc, Some(&data), Some(&mut texture))
                .unwrap();
        }
        let screenshot = read_texture(&device, &context, &texture.unwrap()).unwrap();
        assert_eq!((screenshot.width, screenshot.height), (2, 2));
        assert_eq!(
            screenshot.rgba,
            [
                255, 0, 0, 255, 0, 0, 255, 255, 0, 255, 0, 255, 255, 255, 255, 255
            ]
        );
    }
    #[test]
    fn screenshots_are_valid_pngs_with_original_dimensions_and_no_partial_files() {
        let folder = tempfile::tempdir().unwrap();
        let image = Screenshot {
            width: 2,
            height: 1,
            rgba: vec![255, 0, 0, 255, 0, 0, 255, 255],
        };
        let path = save(&image, folder.path(), "Desktop", None).unwrap();
        let mut reader =
            png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&path).unwrap()))
                .read_info()
                .unwrap();
        assert_eq!((reader.info().width, reader.info().height), (2, 1));
        let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
        reader.next_frame(&mut bytes).unwrap();
        assert_eq!(bytes, image.rgba);
        assert!(!path.with_extension("png.partial").exists());
    }
}
