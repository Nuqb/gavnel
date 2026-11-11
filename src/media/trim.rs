//! Copy compressed H.264/AAC samples when the requested start is a safe keyframe.
use super::*;
use mp4::{
    AacConfig, AvcConfig, Mp4Config, Mp4Reader, Mp4Sample, Mp4Track, Mp4Writer, TrackConfig,
    TrackType,
};
use std::{
    fs::File,
    io::{BufReader, BufWriter, Seek, SeekFrom, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

struct TrackPlan {
    source_id: u32,
    config: TrackConfig,
    first: u32,
    last: u32,
    origin: u64,
    offset: u64,
}

pub(super) fn eligible(project: &EditProject, options: &ExportOptions) -> bool {
    let [segment] = project.segments.as_slice() else {
        return false;
    };
    options.height.is_none()
        && options.size_mb.is_none()
        && options.canvas.is_none()
        && segment.crop.pixels(&segment.info) == (0, 0, segment.info.width, segment.info.height)
        && segment.audio == ClipSegment::new(segment.info.clone()).audio
}

pub(super) fn write(
    project: &EditProject,
    output: &Path,
    progress: &Arc<Mutex<JobProgress>>,
    cancel: &AtomicBool,
) -> Result<()> {
    let segment = &project.segments[0];
    let source = File::open(&segment.info.path)?;
    let mut reader = Mp4Reader::read_header(
        BufReader::new(source.try_clone()?),
        source.metadata()?.len(),
    )?;
    ensure!(
        !reader.is_fragmented(),
        "Fragmented video requires rendering."
    );
    let mut ids: Vec<_> = reader.tracks().keys().copied().collect();
    ids.sort_unstable();
    let plans = ids
        .iter()
        .map(|id| plan(&reader.tracks()[id], segment))
        .collect::<Result<Vec<_>>>()?;
    let config = Mp4Config {
        major_brand: reader.ftyp.major_brand,
        minor_version: reader.ftyp.minor_version,
        compatible_brands: reader.ftyp.compatible_brands.clone(),
        timescale: 1_000_000,
    };
    let mut writer = Mp4Writer::write_start(BufWriter::new(File::create(output)?), &config)?;
    for plan in &plans {
        writer.add_track(&plan.config)?;
    }
    copy_samples(&mut reader, &mut writer, &plans, progress, cancel)?;
    writer.write_end()?;
    writer.into_writer().flush()?;
    set_visible_range(output, &plans, segment.duration())?;
    // Preserve the original default mix and separate audio track names.
    let labels = segment.info.path.with_extension("tracks.json");
    if labels.exists() {
        std::fs::copy(labels, output.with_extension("tracks.json"))?;
    }
    Ok(())
}

fn config(track: &Mp4Track) -> Result<TrackConfig> {
    let mut config = match track.track_type()? {
        TrackType::Video => {
            let avc = track
                .trak
                .mdia
                .minf
                .stbl
                .stsd
                .avc1
                .as_ref()
                .context("Only H.264 supports compressed copy.")?;
            ensure!(
                avc.avcc.length_size_minus_one == 3
                    && avc.avcc.sequence_parameter_sets.len() == 1
                    && avc.avcc.picture_parameter_sets.len() == 1,
                "Unsupported H.264 configuration."
            );
            TrackConfig::from(AvcConfig {
                width: track.width(),
                height: track.height(),
                seq_param_set: track.sequence_parameter_set()?.to_vec(),
                pic_param_set: track.picture_parameter_set()?.to_vec(),
            })
        }
        TrackType::Audio => TrackConfig::from(AacConfig {
            bitrate: track.bitrate(),
            profile: track.audio_profile()?,
            freq_index: track.sample_freq_index()?,
            chan_conf: track.channel_config()?,
        }),
        _ => anyhow::bail!("Unsupported track for compressed copy."),
    };
    config.timescale = track.timescale();
    config.language = track.language().into();
    Ok(config)
}

fn source_offset(track: &Mp4Track) -> Result<u64> {
    let Some(edits) = track.trak.edts.as_ref().and_then(|edts| edts.elst.as_ref()) else {
        return Ok(0);
    };
    let [entry] = edits.entries.as_slice() else {
        anyhow::bail!("Complex edit lists require rendering.");
    };
    ensure!(
        entry.media_time < i64::MAX as u64
            && entry.media_rate == 1
            && entry.media_rate_fraction == 0,
        "Unsupported source edit list."
    );
    Ok(entry.media_time)
}

fn plan(track: &Mp4Track, segment: &ClipSegment) -> Result<TrackPlan> {
    let config = config(track)?;
    let times = sample_times(track)?;
    let start = (segment.start * track.timescale() as f64).round() as u64 + source_offset(track)?;
    let end = (segment.end * track.timescale() as f64).round() as u64 + source_offset(track)?;
    let (first, last) = sample_range(track, &times, start, end)?;
    Ok(TrackPlan {
        source_id: track.track_id(),
        config,
        first: first as u32 + 1,
        last: last as u32,
        origin: times[first],
        offset: start - times[first],
    })
}

fn sample_times(track: &Mp4Track) -> Result<Vec<u64>> {
    let mut times = Vec::with_capacity(track.sample_count() as usize + 1);
    let mut elapsed = 0u64;
    for entry in &track.trak.mdia.minf.stbl.stts.entries {
        for _ in 0..entry.sample_count {
            times.push(elapsed);
            elapsed += entry.sample_delta as u64;
        }
    }
    ensure!(
        times.len() == track.sample_count() as usize && !times.is_empty(),
        "Invalid sample timing."
    );
    times.push(elapsed);
    Ok(times)
}

fn sample_range(track: &Mp4Track, times: &[u64], start: u64, end: u64) -> Result<(usize, usize)> {
    let mut first = times
        .partition_point(|time| *time <= start)
        .saturating_sub(1)
        .min(times.len() - 2);
    let mut last = times
        .partition_point(|time| *time < end)
        .min(times.len() - 1);
    let reordered = track
        .trak
        .mdia
        .minf
        .stbl
        .ctts
        .as_ref()
        .is_some_and(|ctts| ctts.entries.iter().any(|entry| entry.sample_offset != 0));
    if track.track_type()? == TrackType::Video
        && let Some(sync) = &track.trak.mdia.minf.stbl.stss
    {
        ensure!(
            sync.entries
                .iter()
                .all(|id| *id > 0 && (*id as usize) < times.len()),
            "Invalid keyframe table."
        );
        first = sync
            .entries
            .iter()
            .rfind(|id| (**id as usize) <= first + 1)
            .copied()
            .context("No preceding keyframe.")? as usize
            - 1;
        if reordered {
            last = sync
                .entries
                .iter()
                .find(|id| times[**id as usize - 1] >= end)
                .map(|id| *id as usize - 1)
                .unwrap_or(times.len() - 1);
        }
    }
    if track.track_type()? == TrackType::Video {
        // Windows readers do not consistently discard pre-roll from edit lists.
        // Preserve exact cuts by rendering when a leading partial GOP is needed.
        ensure!(
            start.abs_diff(times[first]) <= 1 && (!reordered || start == 0),
            "This cut needs frame-accurate rendering."
        );
    }
    ensure!(
        first < last && start >= times[first],
        "Empty trim selection."
    );
    Ok((first, last))
}

fn copy_samples(
    reader: &mut Mp4Reader<BufReader<File>>,
    writer: &mut Mp4Writer<BufWriter<File>>,
    plans: &[TrackPlan],
    progress: &Arc<Mutex<JobProgress>>,
    cancel: &AtomicBool,
) -> Result<()> {
    let total: u64 = plans
        .iter()
        .map(|plan| (plan.last - plan.first + 1) as u64)
        .sum();
    let mut copied = 0;
    progress.lock().unwrap().message = "Saving video".into();
    for (index, plan) in plans.iter().enumerate() {
        for id in plan.first..=plan.last {
            ensure!(!cancel.load(Ordering::Acquire), "Cancelled");
            let mut sample: Mp4Sample = reader
                .read_sample(plan.source_id, id)?
                .context("Missing media sample.")?;
            sample.start_time -= plan.origin;
            writer.write_sample(index as u32 + 1, &sample)?;
            copied += 1;
            if copied % 32 == 0 || copied == total {
                progress.lock().unwrap().fraction = copied as f32 / total as f32 * 0.94;
            }
        }
    }
    Ok(())
}

fn set_visible_range(path: &Path, plans: &[TrackPlan], duration: f64) -> Result<()> {
    use mp4::{BoxHeader, BoxType, ReadBox, WriteBox};
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?;
    let mut position = 0;
    let length = file.metadata()?.len();
    while position < length {
        file.seek(SeekFrom::Start(position))?;
        let header = BoxHeader::read(&mut file)?;
        if header.name == BoxType::MoovBox {
            let mut moov = mp4::MoovBox::read_box(&mut file, header.size)?;
            let ticks = (duration * moov.mvhd.timescale as f64).round() as u64;
            moov.mvhd.duration = ticks;
            moov.mvhd.version = 1;
            for (track, plan) in moov.traks.iter_mut().zip(plans) {
                track.tkhd.duration = ticks;
                track.tkhd.version = 1;
                let edits = track.edts.get_or_insert_with(Default::default);
                let list = edits.elst.get_or_insert_with(Default::default);
                list.version = 1;
                list.entries.clear();
                list.entries.push(Default::default());
                list.entries[0].segment_duration = ticks;
                list.entries[0].media_time = plan.offset;
                list.entries[0].media_rate = 1;
            }
            file.seek(SeekFrom::Start(position))?;
            file.set_len(position)?;
            moov.write_box(&mut file)?;
            return Ok(());
        }
        ensure!(header.size > 0, "Invalid MP4 box size.");
        position += header.size;
    }
    anyhow::bail!("Missing MP4 index.")
}
