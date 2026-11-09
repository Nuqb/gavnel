//! Offline audio inspection; never opens a capture endpoint or plays sound.
use anyhow::Result;
use std::path::Path;
use windows::{Win32::Media::MediaFoundation::*, core::PCWSTR};

pub fn decoded_frames(path: &Path, seconds: u64) -> Result<Vec<[f32; 2]>> {
    unsafe {
        MFStartup(MF_VERSION, MFSTARTUP_FULL)?;
        let result = (|| -> Result<Vec<[f32; 2]>> {
            let path = std::fs::canonicalize(path)?;
            let path = path.to_string_lossy();
            let path = path.strip_prefix("\\\\?\\").unwrap_or(&path);
            let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
            let reader = MFCreateSourceReaderFromURL(PCWSTR(wide.as_ptr()), None)?;
            let index = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
            reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
            reader.SetStreamSelection(index, true)?;
            let media = MFCreateMediaType()?;
            media.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
            media.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_Float)?;
            media.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
            media.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48_000)?;
            media.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 32)?;
            reader.SetCurrentMediaType(index, None, &media)?;
            let mut frames = Vec::new();
            loop {
                let mut flags = 0;
                let mut sample = None;
                reader.ReadSample(index, 0, None, Some(&mut flags), None, Some(&mut sample))?;
                if let Some(sample) = sample {
                    let buffer = sample.ConvertToContiguousBuffer()?;
                    let mut ptr = std::ptr::null_mut();
                    let mut len = 0;
                    buffer.Lock(&mut ptr, None, Some(&mut len))?;
                    let data = std::slice::from_raw_parts(ptr, len as usize);
                    for bytes in data.as_chunks::<8>().0 {
                        frames.push([
                            f32::from_le_bytes(bytes[..4].try_into().unwrap()),
                            f32::from_le_bytes(bytes[4..].try_into().unwrap()),
                        ]);
                        if frames.len() as u64 >= seconds * 48_000 {
                            break;
                        }
                    }
                    buffer.Unlock()?;
                }
                if frames.len() as u64 >= seconds * 48_000
                    || flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0
                {
                    break;
                }
            }
            anyhow::ensure!(!frames.is_empty(), "No audio samples decoded");
            Ok(frames)
        })();
        let _ = MFShutdown();
        result
    }
}

pub fn inspect(path: &Path, seconds: u64) -> Result<()> {
    let frames = decoded_frames(path, seconds)?;
    let db = |value: f64| 20.0 * value.max(1e-10).log10();
    let mut previous = [0.0f64; 2];
    for (second, block) in frames.chunks(48_000).enumerate() {
        let mut squares = [0.0f64; 2];
        let mut differences = [0.0f64; 2];
        let mut sums = [0.0f64; 2];
        let mut peak = [0.0f64; 2];
        let mut clipped = [0u64; 2];
        let mut cross = 0.0;
        for frame in block {
            let values = [frame[0] as f64, frame[1] as f64];
            for ch in 0..2 {
                sums[ch] += values[ch];
                squares[ch] += values[ch] * values[ch];
                differences[ch] += (values[ch] - previous[ch]).powi(2);
                peak[ch] = peak[ch].max(values[ch].abs());
                clipped[ch] += u64::from(values[ch].abs() >= 0.999);
            }
            cross += values[0] * values[1];
            previous = values;
        }
        let n = block.len() as f64;
        println!(
            "second {}: RMS L/R {:.1}/{:.1} dBFS; peak {:.1}/{:.1}; difference RMS {:.1}/{:.1}; DC {:.5}/{:.5}; clipped {}/{}; correlation {:.3}",
            second + 1,
            db((squares[0] / n).sqrt()),
            db((squares[1] / n).sqrt()),
            db(peak[0]),
            db(peak[1]),
            db((differences[0] / n).sqrt()),
            db((differences[1] / n).sqrt()),
            sums[0] / n,
            sums[1] / n,
            clipped[0],
            clipped[1],
            cross / (squares[0] * squares[1]).sqrt().max(1e-20)
        );
    }
    Ok(())
}
