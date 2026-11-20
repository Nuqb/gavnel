use super::{native::*, *};
use windows::{
    Win32::{Media::MediaFoundation::*, System::Com::StructuredStorage::*},
    core::GUID,
};

pub fn probe(path: &Path) -> Result<MediaInfo> {
    let source = Source::open(path)?;
    let (video, mut audio) = streams(&source)?;
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

/// Audio stream indices in file order, given each stream's major type as a
/// Windows component lists them. The MP4 source reader lists tracks in reverse
/// file order while the media engine keeps file order; recordings store video
/// first, so finding video last identifies a reversed listing.
pub fn audio_in_file_order(majors: &[GUID]) -> Vec<u32> {
    let mut audio: Vec<u32> = (0..majors.len() as u32)
        .filter(|&index| majors[index as usize] == MFMediaType_Audio)
        .collect();
    let video = majors.iter().position(|major| *major == MFMediaType_Video);
    if majors.len() > 1 && video == Some(majors.len() - 1) {
        audio.reverse();
    }
    audio
}

/// The first video stream's native type and every audio stream, with audio in
/// file order (the order separate-track labels are stored in).
fn streams(source: &Source) -> Result<(Option<IMFMediaType>, Vec<AudioTrack>)> {
    let (mut video, mut majors) = (None, Vec::new());
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
        majors.push(major);
    }
    let audio = audio_in_file_order(&majors)
        .into_iter()
        .enumerate()
        .map(|(number, index)| AudioTrack {
            index,
            label: format!("Audio {}", number + 1),
        })
        .collect();
    Ok((video, audio))
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
    // The editor mixes the separate tracks itself, so it hides the stored mix.
    if audio.len() > 1
        && let Some(combined) = audio.iter().position(|track| track.label == "Combined")
    {
        audio.remove(combined);
    }
}

/// File-order position of the combined mix among the audio tracks, when the
/// clip also has separate tracks.
pub fn combined_audio_position(path: &Path) -> Result<Option<usize>> {
    let (_, audio) = streams(&Source::open(path)?)?;
    let mut separate = audio.clone();
    label_tracks(path, &mut separate);
    if separate.len() == audio.len() {
        return Ok(None);
    }
    Ok(audio
        .iter()
        .position(|track| !separate.iter().any(|kept| kept.index == track.index)))
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

#[cfg(test)]
mod tests {
    use super::*;
    fn tracks(count: u32) -> Vec<AudioTrack> {
        (0..count)
            .map(|i| AudioTrack {
                index: i + 1,
                label: format!("Audio {}", i + 1),
            })
            .collect()
    }
    #[test]
    fn audio_order_follows_file_order_in_both_windows_listings() {
        let (v, a) = (MFMediaType_Video, MFMediaType_Audio);
        // Media engine: file order, video first.
        assert_eq!(audio_in_file_order(&[v, a, a, a]), [1, 2, 3]);
        // Source reader: reverse file order, video last.
        assert_eq!(audio_in_file_order(&[a, a, a, v]), [2, 1, 0]);
        assert_eq!(audio_in_file_order(&[a]), [0]);
    }
    #[test]
    fn editor_hides_combined_mix_whether_stored_first_or_last() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clip.mp4");
        for (labels, kept) in [
            (r#"["Combined","PC Audio","Microphone"]"#, [2, 3]),
            (r#"["PC Audio","Microphone","Combined"]"#, [1, 2]),
        ] {
            std::fs::write(path.with_extension("tracks.json"), labels).unwrap();
            let mut audio = tracks(3);
            label_tracks(&path, &mut audio);
            let indices: Vec<_> = audio.iter().map(|track| track.index).collect();
            assert_eq!(indices, kept);
            assert!(audio.iter().all(|track| track.label != "Combined"));
        }
    }
}
