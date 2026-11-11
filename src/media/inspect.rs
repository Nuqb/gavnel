use super::{native::*, *};
use windows::Win32::{Media::MediaFoundation::*, System::Com::StructuredStorage::*};

pub fn probe(path: &Path) -> Result<MediaInfo> {
    let source = Source::open(path)?;
    let (mut video, mut audio) = (None, Vec::new());
    for index in 0..1024 {
        let media = match unsafe { source.reader.GetNativeMediaType(index, 0) } {
            Ok(media) => media,
            Err(error) if error.code() == MF_E_INVALIDSTREAMNUMBER => break,
            Err(error) => return Err(error.into()),
        };
        let major = unsafe { media.GetGUID(&MF_MT_MAJOR_TYPE)? };
        if major == MFMediaType_Video && video.is_none() {
            video = Some(media);
        }
        if major == MFMediaType_Audio {
            audio.push(AudioTrack {
                index,
                label: format!("Audio {}", audio.len() + 1),
            });
        }
    }
    let video = video.context("Choose a video file.")?;
    let (size, rate, duration) = unsafe {
        let mut value = source
            .reader
            .GetPresentationAttribute(MF_SOURCE_READER_MEDIASOURCE.0 as u32, &MF_PD_DURATION)?;
        let duration = PropVariantToInt64(&value);
        let _ = PropVariantClear(&mut value);
        (
            video.GetUINT64(&MF_MT_FRAME_SIZE)?,
            video
                .GetUINT64(&MF_MT_FRAME_RATE)
                .unwrap_or(30u64 << 32 | 1),
            duration? as f64 / TICKS,
        )
    };
    let (width, height) = ((size >> 32) as u32, size as u32);
    let fps = (rate >> 32) as f64 / (rate as u32) as f64;
    ensure!(
        width >= 2 && height >= 2 && width <= 16384 && height <= 16384 && duration > 0.0,
        "Invalid video dimensions or duration."
    );
    label_tracks(path, &mut audio);
    Ok(MediaInfo {
        path: path.into(),
        duration,
        width,
        height,
        fps: if fps.is_finite() && fps > 0.0 {
            fps
        } else {
            30.0
        },
        audio,
        markers: markers(path)?,
    })
}

fn label_tracks(path: &Path, audio: &mut Vec<AudioTrack>) {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let legacy = audio.len() == 3
        && stem.split('_').any(|part| {
            part.len() == 10
                && part.as_bytes().get(4) == Some(&b'-')
                && part.as_bytes().get(7) == Some(&b'-')
        });
    if legacy {
        for (track, label) in audio.iter_mut().zip(["Combined", "PC Audio", "Microphone"]) {
            track.label = label.into();
        }
    }
    if let Ok(bytes) = std::fs::read(path.with_extension("tracks.json"))
        && let Ok(labels) = serde_json::from_slice::<Vec<String>>(&bytes)
    {
        for (track, label) in audio.iter_mut().zip(labels) {
            track.label = label;
        }
    }
    if audio.len() > 1 && audio[0].label == "Combined" {
        audio.remove(0);
    }
}

fn markers(path: &Path) -> Result<Vec<(f64, String)>> {
    let events = crate::highlights::load_events(path);
    let mut markers: Vec<_> = crate::bookmarks::load(path)?
        .into_iter()
        .map(|time| {
            let label = events
                .iter()
                .find(|event| (event.ticks as f64 / TICKS - time).abs() < 0.01)
                .map(|event| event.kind.label().to_owned())
                .unwrap_or_else(|| "Bookmark".into());
            (time, label)
        })
        .collect();
    for event in events {
        let seconds = event.ticks as f64 / TICKS;
        if !markers
            .iter()
            .any(|(time, _)| (time - seconds).abs() < 0.01)
        {
            markers.push((seconds, event.kind.label().into()));
        }
    }
    markers.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(markers)
}
