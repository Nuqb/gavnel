use super::{native::*, *};
use eframe::egui::ColorImage;
use windows::{Win32::Media::MediaFoundation::*, core::Interface};

pub(super) struct Raster {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
impl Raster {
    /// Bilinear crop/scale into a centered, letterboxed top-down RGB32 surface.
    pub fn render(&self, crop: (u32, u32, u32, u32), canvas: (u32, u32)) -> Vec<u8> {
        let (left, top, width, height) = crop;
        let (out_w, out_h) = canvas;
        if canvas == (width, height) {
            let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
            for y in top..top + height {
                let start = ((y * self.width + left) * 4) as usize;
                for pixel in self.rgba[start..start + width as usize * 4]
                    .as_chunks::<4>()
                    .0
                {
                    pixels.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
                }
            }
            return pixels;
        }
        let scale = (out_w as f64 / width as f64)
            .min(out_h as f64 / height as f64)
            .min(1.0);
        let (w, h) = (
            (width as f64 * scale).round() as u32,
            (height as f64 * scale).round() as u32,
        );
        let (x0, y0) = ((out_w - w) / 2, (out_h - h) / 2);
        let mut pixels = vec![0; out_w as usize * out_h as usize * 4];
        for y in 0..h {
            let sy =
                ((y as f64 + 0.5) * height as f64 / h as f64 - 0.5).clamp(0.0, (height - 1) as f64);
            for x in 0..w {
                let sx = ((x as f64 + 0.5) * width as f64 / w as f64 - 0.5)
                    .clamp(0.0, (width - 1) as f64);
                let pixel = self.interpolate(left as f64 + sx, top as f64 + sy);
                let at = (((y + y0) * out_w + x + x0) * 4) as usize;
                pixels[at..at + 4].copy_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
            }
        }
        pixels
    }
    fn interpolate(&self, x: f64, y: f64) -> [u8; 3] {
        let (x0, y0) = (x.floor() as u32, y.floor() as u32);
        let (x1, y1) = ((x0 + 1).min(self.width - 1), (y0 + 1).min(self.height - 1));
        let (dx, dy) = (x - x0 as f64, y - y0 as f64);
        std::array::from_fn(|channel| {
            let value = |px, py| self.rgba[((py * self.width + px) * 4) as usize + channel] as f64;
            ((value(x0, y0) * (1.0 - dx) + value(x1, y0) * dx) * (1.0 - dy)
                + (value(x0, y1) * (1.0 - dx) + value(x1, y1) * dx) * dy)
                .round() as u8
        })
    }
    pub fn image(&self, width: u32) -> ColorImage {
        let width = width.min(self.width);
        let height =
            ((width as f64 * self.height as f64 / self.width as f64).round() as u32).max(1);
        let mut bytes = self.render((0, 0, self.width, self.height), (width, height));
        for pixel in bytes.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &bytes)
    }
}

pub(super) struct Video {
    source: Source,
    info: MediaInfo,
    stride: i32,
    current: Option<(i64, Raster)>,
}
impl Video {
    pub fn new(info: &MediaInfo) -> Result<Self> {
        let source = Source::open(&info.path)?;
        let index = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
        source.select(
            index,
            &media_type(&MFMediaType_Video, &MFVideoFormat_RGB32)?,
        )?;
        let media = unsafe { source.reader.GetCurrentMediaType(index)? };
        let stride = unsafe {
            media
                .GetUINT32(&MF_MT_DEFAULT_STRIDE)
                .map(|value| value as i32)
                .unwrap_or(-(info.width as i32 * 4))
        };
        Ok(Self {
            source,
            info: info.clone(),
            stride,
            current: None,
        })
    }
    pub fn seek(&mut self, time: f64) -> Result<()> {
        self.source.seek(time)?;
        self.current = None;
        Ok(())
    }
    pub fn at(&mut self, seconds: f64) -> Result<&Raster> {
        let target = seconds * TICKS - TICKS * 0.5 / self.info.fps;
        if self
            .current
            .as_ref()
            .is_none_or(|(time, _)| (*time as f64) < target)
        {
            // Seeking can land before the requested timestamp. Let Windows
            // advance the decoder, but copy pixels only for the frame we keep.
            let mut latest = None;
            while let Some((time, sample)) = self
                .source
                .read(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32)?
            {
                latest = Some((time, sample));
                if time as f64 >= target {
                    break;
                }
            }
            if let Some((time, sample)) = latest {
                self.current = Some((time, self.raster(&sample)?));
            }
        }
        self.current
            .as_ref()
            .map(|(_, raster)| raster)
            .context("Windows returned no video frames.")
    }

    fn raster(&mut self, sample: &IMFSample) -> Result<Raster> {
        let media = unsafe {
            self.source
                .reader
                .GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32)?
        };
        let size = unsafe { media.GetUINT64(&MF_MT_FRAME_SIZE)? };
        let subtype = unsafe { media.GetGUID(&MF_MT_SUBTYPE)? };
        ensure!(
            (size >> 32) as u32 >= self.info.width
                && (size >> 32) as u32 <= self.info.width.div_ceil(16) * 16
                && size as u32 >= self.info.height
                && size as u32 <= self.info.height.div_ceil(16) * 16
                && subtype == MFVideoFormat_RGB32,
            "Unexpected decoded video layout: {}x{}, {subtype:?}.",
            size >> 32,
            size as u32
        );
        self.stride = unsafe {
            media
                .GetUINT32(&MF_MT_DEFAULT_STRIDE)
                .map(|value| value as i32)
                .unwrap_or(self.stride)
        };
        decode(sample, &self.info, (self.stride, size as u32))
    }
}

fn decode(sample: &IMFSample, info: &MediaInfo, layout: (i32, u32)) -> Result<Raster> {
    let (stride, storage_height) = layout;
    let buffer = unsafe { sample.GetBufferByIndex(0)? };
    let mut rgba = vec![0; info.width as usize * info.height as usize * 4];
    if let Ok(buffer) = buffer.cast::<IMF2DBuffer>() {
        let (mut scanline, mut pitch) = (std::ptr::null_mut(), 0);
        unsafe {
            buffer.Lock2D(&mut scanline, &mut pitch)?;
        }
        let result = copy_rows(scanline, pitch, info, &mut rgba);
        unsafe {
            buffer.Unlock2D()?;
        }
        result?;
    } else {
        let bytes = sample_bytes(sample)?;
        let extent = stride.unsigned_abs() as usize * (storage_height as usize - 1)
            + info.width as usize * 4;
        ensure!(
            bytes.len() >= extent,
            "Windows returned an incomplete video frame."
        );
        let offset = if stride < 0 {
            stride.unsigned_abs() as usize * (storage_height as usize - 1)
        } else {
            0
        };
        copy_rows(
            unsafe { bytes.as_ptr().add(offset) },
            stride,
            info,
            &mut rgba,
        )?;
    }
    Ok(Raster {
        width: info.width,
        height: info.height,
        rgba,
    })
}
fn copy_rows(top: *const u8, pitch: i32, info: &MediaInfo, rgba: &mut [u8]) -> Result<()> {
    ensure!(
        pitch.unsigned_abs() >= info.width * 4,
        "Invalid decoded video stride."
    );
    for y in 0..info.height as usize {
        // Lock2D supplies the displayed top row and a signed pitch. Contiguous
        // buffers are length-checked above and adjusted to the displayed top row.
        let row = unsafe {
            std::slice::from_raw_parts(
                top.offset(y as isize * pitch as isize),
                info.width as usize * 4,
            )
        };
        for (x, bgra) in row.as_chunks::<4>().0.iter().enumerate() {
            let at = (y * info.width as usize + x) * 4;
            rgba[at..at + 4].copy_from_slice(&[bgra[2], bgra[1], bgra[0], 255]);
        }
    }
    Ok(())
}

pub fn frame(path: &Path, seconds: f64) -> Result<ColorImage> {
    let info = super::inspect::probe(path)?;
    let mut video = Video::new(&info)?;
    let seconds = seconds.clamp(0.0, (info.duration - 0.5 / info.fps).max(0.0));
    video.seek(seconds)?;
    Ok(video.at(seconds)?.image(800))
}
pub fn filmstrip(path: &Path) -> Result<Vec<ColorImage>> {
    filmstrip_progressive(path, |_| {})
}

pub(super) fn filmstrip_progressive(
    path: &Path,
    mut ready: impl FnMut(&[ColorImage]),
) -> Result<Vec<ColorImage>> {
    let info = super::inspect::probe(path)?;
    let mut video = Video::new(&info)?;
    let mut images = Vec::with_capacity(8);
    for index in 0..8 {
        let time = info.duration * (index as f64 + 0.5) / 8.0;
        video.seek(time)?;
        images.push(video.at(time)?.image(144));
        ready(&images);
    }
    Ok(images)
}
