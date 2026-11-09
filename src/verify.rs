use anyhow::{Context, Result};
use std::path::Path;
use windows::{Win32::Media::MediaFoundation::*, core::PCWSTR};

// Offline verification only. The recorder never maps raw video to the CPU.
pub fn clip(path: &Path) -> Result<()> {
    unsafe {
        MFStartup(MF_VERSION, MFSTARTUP_FULL)?;
        let path = std::fs::canonicalize(path)?;
        let path = path.to_string_lossy();
        let path = path.strip_prefix("\\\\?\\").unwrap_or(&path);
        let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
        for (name, index, subtype) in [
            (
                "video",
                MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                MFVideoFormat_NV12,
            ),
            (
                "audio",
                MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32,
                MFAudioFormat_PCM,
            ),
        ] {
            let reader = MFCreateSourceReaderFromURL(PCWSTR(wide.as_ptr()), None)?;
            if reader.GetNativeMediaType(index, 0).is_err() {
                println!("{name}: no track");
                continue;
            }
            reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
            reader.SetStreamSelection(index, true)?;
            let media = MFCreateMediaType()?;
            media.SetGUID(
                &MF_MT_MAJOR_TYPE,
                if name == "video" {
                    &MFMediaType_Video
                } else {
                    &MFMediaType_Audio
                },
            )?;
            media.SetGUID(&MF_MT_SUBTYPE, &subtype)?;
            reader
                .SetCurrentMediaType(index, None, &media)
                .with_context(|| format!("Could not initialize {name} decoder"))?;
            let mut count = 0u64;
            let mut last = -1i64;
            let mut end = 0i64;
            let mut nonzero = 0u64;
            let mut total = 0u64;
            let mut previous = Vec::new();
            let mut repeats = 0u64;
            let mut gaps = Vec::new();
            let mut changed_per_second = std::collections::BTreeMap::<i64, u64>::new();
            loop {
                let mut flags = 0;
                let mut pts = 0;
                let mut sample = None;
                reader.ReadSample(
                    index,
                    0,
                    None,
                    Some(&mut flags),
                    Some(&mut pts),
                    Some(&mut sample),
                )?;
                if let Some(sample) = sample {
                    if pts < last {
                        anyhow::bail!("{name} timestamps went backwards");
                    }
                    if last >= 0 {
                        gaps.push((pts - last) as f64 / 10_000.0);
                    }
                    last = pts;
                    end = pts + sample.GetSampleDuration().unwrap_or(0);
                    count += 1;
                    if count < 4 || name == "video" {
                        let buffer = sample.ConvertToContiguousBuffer()?;
                        let mut ptr = std::ptr::null_mut();
                        let mut len = 0;
                        buffer.Lock(&mut ptr, None, Some(&mut len))?;
                        let data = std::slice::from_raw_parts(ptr, len as usize);
                        if count < 4 {
                            nonzero += data.iter().filter(|b| **b > 20).count() as u64;
                            total += len as u64;
                        }
                        if name == "video" {
                            let luma = data.get(..data.len() * 2 / 3).unwrap_or(data);
                            let sampled: Vec<u8> = luma
                                .iter()
                                .step_by((luma.len() / 8192).max(1))
                                .copied()
                                .collect();
                            if previous.len() == sampled.len() {
                                let difference: u64 = sampled
                                    .iter()
                                    .zip(&previous)
                                    .map(|(a, b)| a.abs_diff(*b) as u64)
                                    .sum();
                                if difference as f64 / (sampled.len() as f64) < 0.6 {
                                    repeats += 1;
                                } else {
                                    *changed_per_second.entry(pts / 10_000_000).or_default() += 1;
                                }
                            }
                            previous = sampled;
                        }
                        buffer.Unlock()?;
                    }
                }
                if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                    break;
                }
                if count > 100_000 {
                    anyhow::bail!("Clip verification exceeded its frame limit");
                }
            }
            if count == 0 {
                anyhow::bail!("The {name} track could not be decoded");
            }
            println!(
                "{name}: decoded {count} samples, end {:.3}s, non-dark bytes {nonzero}/{total}",
                end as f64 / 10_000_000.0
            );
            if name == "video" {
                gaps.sort_by(f64::total_cmp);
                println!(
                    "video cadence: {:.2} frames/s, {repeats}/{} near-identical consecutive frames; interval median {:.2}ms, p99 {:.2}ms, >25ms {}, >50ms {}",
                    count as f64 / (end as f64 / 10_000_000.0),
                    count.saturating_sub(1),
                    gaps.get(gaps.len() / 2).unwrap_or(&0.0),
                    gaps.get(gaps.len().saturating_sub(1) * 99 / 100)
                        .unwrap_or(&0.0),
                    gaps.iter().filter(|v| **v > 25.0).count(),
                    gaps.iter().filter(|v| **v > 50.0).count()
                );
                println!(
                    "Changed frames per second (static scenes can legitimately repeat): {changed_per_second:?}"
                );
            }
        }
        MFShutdown()?;
        Ok(())
    }
}
