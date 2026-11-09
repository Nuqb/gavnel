//! Explicit audio diagnostic commands and WAV export.
use super::*;
pub fn format_report() -> Result<()> {
    for device in enumerate_devices(false)? {
        println!(
            "Automatic PC audio output: {} ({})",
            device.label, device.id
        );
    }
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        for (label, flow) in [("PC audio", eRender), ("Microphone", eCapture)] {
            let device = enumerator.GetDefaultAudioEndpoint(flow, eConsole)?;
            let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
            let ptr = client.GetMixFormat()?;
            let format = *ptr;
            let tag = format.wFormatTag;
            let channels = format.nChannels;
            let rate = format.nSamplesPerSec;
            let bits = format.wBitsPerSample;
            let align = format.nBlockAlign;
            println!(
                "{label}: format {tag:#x}, {rate} Hz, {channels} channels, {bits} bits, {align} bytes/frame"
            );
            if tag == 0xfffe && format.cbSize >= 22 {
                let extension = std::ptr::read_unaligned(ptr.cast::<WAVEFORMATEXTENSIBLE>());
                let subtype = extension.SubFormat;
                println!("{label} subtype: {subtype:?}");
            }
            CoTaskMemFree(Some(ptr.cast()));
        }
        Ok(())
    }
}

pub fn diagnostic_pc(folder: &std::path::Path, seconds: u64) -> Result<()> {
    use std::io::Write;
    std::fs::create_dir_all(folder)?;
    let mut sources = Vec::new();
    let mut report = std::fs::File::create(folder.join("devices.txt"))?;
    for device in enumerate_devices(false)? {
        match Endpoint::open(false, &device.id) {
            Ok(mut endpoint) => {
                let mut trace = std::io::BufWriter::new(std::fs::File::create(
                    folder.join(format!("pc-{}-packets.csv", sources.len() + 1)),
                )?);
                writeln!(
                    trace,
                    "device_frame,frames,qpc_frame,placed_frame,flags,clock_frame"
                )?;
                endpoint.trace = Some(trace);
                writeln!(report, "pc-{}-raw.wav: {}", sources.len() + 1, device.label)?;
                sources.push((device.label, endpoint, Vec::new()));
            }
            Err(error) => {
                writeln!(report, "Unavailable: {}: {error:#}", device.label)?;
            }
        }
    }
    anyhow::ensure!(
        !sources.is_empty(),
        "No PC playback devices could be opened"
    );
    let clock = Clock::new()?;
    let mut mixed = Vec::new();
    let mut position = 0u64;
    while clock.now() < seconds * TICKS {
        for (_, endpoint, raw) in &mut sources {
            let retained = endpoint.segments.len();
            endpoint.read(&clock)?;
            for segment in endpoint.segments.iter().skip(retained) {
                raw.extend_from_slice(&segment.frames);
            }
        }
        let ready = (clock.now() * 48_000 / TICKS).saturating_sub(3840);
        while position + 1024 <= ready {
            let mut block = [[0.0; 2]; 1024];
            for (_, endpoint, _) in &mut sources {
                endpoint.mix(position as i64, &mut block, 1.0, None);
            }
            mixed.extend_from_slice(&block);
            position += 1024;
        }
        std::thread::sleep(Duration::from_millis(4));
    }
    for (index, (label, _, frames)) in sources.iter().enumerate() {
        write_float_wave(&folder.join(format!("pc-{}-raw.wav", index + 1)), frames)?;
        let peak = frames
            .iter()
            .flatten()
            .map(|value| value.abs())
            .fold(0.0f32, f32::max);
        println!("{label}: {} frames, peak {peak:.6}", frames.len());
    }
    write_float_wave(&folder.join("pc-all.wav"), &mixed)?;
    println!(
        "PC-only diagnostic: {} ({:.2} seconds)",
        folder.display(),
        mixed.len() as f64 / 48_000.0
    );
    Ok(())
}

/// Explicit diagnostic only: captures inputs separately; no video or sound playback.
pub fn diagnostic(folder: &std::path::Path, seconds: u64) -> Result<()> {
    use std::io::Write;
    let (settings, _) = Settings::load_preserving_storage();
    let clock = Clock::new()?;
    let mut desktop = Endpoint::open(false, "").context("Could not open PC audio")?;
    let mut mic =
        Endpoint::open(true, &settings.input_device).context("Could not open microphone")?;
    std::fs::create_dir_all(folder)?;
    for (name, endpoint) in [("pc", &mut desktop), ("mic", &mut mic)] {
        let mut file = std::io::BufWriter::new(std::fs::File::create(
            folder.join(format!("{name}-packets.csv")),
        )?);
        writeln!(
            file,
            "device_frame,frames,qpc_frame,placed_frame,flags,clock_frame"
        )?;
        endpoint.trace = Some(file);
    }
    let mut raw_desktop = Vec::new();
    let mut raw_mic = Vec::new();
    let mut mixed = Vec::new();
    let mut position = 0u64;
    while clock.now() < seconds * TICKS {
        for (endpoint, raw) in [(&mut desktop, &mut raw_desktop), (&mut mic, &mut raw_mic)] {
            let retained = endpoint.segments.len();
            endpoint.read(&clock)?;
            for segment in endpoint.segments.iter().skip(retained) {
                raw.extend_from_slice(&segment.frames);
            }
        }
        let ready = (clock.now() * 48_000 / TICKS).saturating_sub(3840);
        while position + 1024 <= ready {
            let mut block = [[0.0; 2]; 1024];
            desktop.mix(position as i64, &mut block, settings.desktop_volume, None);
            mic.mix(
                position as i64,
                &mut block,
                settings.mic_volume,
                settings.mic_mono.then_some(settings.mic_channel),
            );
            mixed.extend_from_slice(&block);
            position += 1024;
        }
        std::thread::sleep(Duration::from_millis(4));
    }
    drop(desktop);
    drop(mic);
    for (name, frames) in [
        ("pc-raw.wav", raw_desktop),
        ("mic-raw.wav", raw_mic),
        ("mixed.wav", mixed),
    ] {
        write_float_wave(&folder.join(name), &frames)?;
        println!(
            "Saved {name}: {:.2} seconds",
            frames.len() as f64 / 48_000.0
        );
    }
    println!("Audio diagnostic: {}", folder.display());
    Ok(())
}

pub(super) fn write_float_wave(path: &std::path::Path, frames: &[[f32; 2]]) -> Result<()> {
    use std::io::Write;
    let bytes = u32::try_from(frames.len() * 8)?;
    let mut file = std::io::BufWriter::new(std::fs::File::create(path)?);
    file.write_all(b"RIFF")?;
    file.write_all(&(48 + bytes).to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16u32.to_le_bytes())?;
    file.write_all(&3u16.to_le_bytes())?;
    file.write_all(&2u16.to_le_bytes())?;
    file.write_all(&48_000u32.to_le_bytes())?;
    file.write_all(&384_000u32.to_le_bytes())?;
    file.write_all(&8u16.to_le_bytes())?;
    file.write_all(&32u16.to_le_bytes())?;
    file.write_all(b"fact")?;
    file.write_all(&4u32.to_le_bytes())?;
    file.write_all(&(frames.len() as u32).to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&bytes.to_le_bytes())?;
    for frame in frames {
        for value in frame {
            file.write_all(&value.to_le_bytes())?;
        }
    }
    file.flush()?;
    Ok(())
}

/// Encode an existing diagnostic waveform through the production AAC path.
pub fn reencode_diagnostic(source: &std::path::Path, output: &std::path::Path) -> Result<()> {
    let frames = crate::audio_check::decoded_frames(source, 60)?;
    let result = (|| -> Result<()> {
        let mut encoder = AacEncoder::new()?;
        let budget = Budget::new(4 * 1024 * 1024);
        let (tx, rx) = crossbeam_channel::bounded(256);
        let mut writer = mp4::Mp4Writer::write_start(
            std::io::BufWriter::new(std::fs::File::create(output)?),
            &mp4::Mp4Config {
                major_brand: "isom".parse()?,
                minor_version: 0,
                compatible_brands: vec!["isom".parse()?],
                timescale: 720_000,
            },
        )?;
        let mut track: mp4::TrackConfig = mp4::AacConfig {
            bitrate: 192_000,
            ..Default::default()
        }
        .into();
        track.timescale = 48_000;
        writer.add_track(&track)?;
        for (index, block) in frames.chunks(1024).enumerate() {
            encoder.encode(block, index as u64 * 1024 * TICKS / 48_000, &budget, &tx)?;
            for packet in rx.try_iter() {
                writer.write_sample(
                    1,
                    &mp4::Mp4Sample {
                        start_time: packet.pts * 48_000 / TICKS,
                        duration: 1024,
                        rendering_offset: 0,
                        is_sync: true,
                        bytes: bytes::Bytes::copy_from_slice(&packet.data.bytes),
                    },
                )?;
            }
        }
        writer.write_end()?;
        drop(writer);
        Ok(())
    })();
    result?;
    println!("Saved encoded diagnostic: {}", output.display());
    Ok(())
}
