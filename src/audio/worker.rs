//! Audio capture/mixing worker and thread-local COM ownership.
use super::*;
struct ComApartment;
impl ComApartment {
    fn new() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        }
        Ok(Self)
    }
}
impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
pub(super) fn run(
    settings: Settings,
    clock: Clock,
    output: crate::replay::EncoderOutput,
    signal: Arc<AtomicBool>,
) -> Result<()> {
    let crate::replay::EncoderOutput {
        budget,
        packets,
        messages: warnings,
    } = output;
    let _apartment = ComApartment::new()?;
    let mut encoder = AacEncoder::new().context("Windows AAC audio encoding could not start")?;
    let mut pc_encoder = if settings.desktop_audio
        && settings.separate_audio_tracks
        && !settings.application_audio()
    {
        Some(AacEncoder::new()?)
    } else {
        None
    };
    let mut mic_encoder = if settings.microphone && settings.separate_audio_tracks {
        Some(AacEncoder::new()?)
    } else {
        None
    };
    if let Some(e) = &mut pc_encoder {
        e.stream = Stream::Desktop;
    }
    if let Some(e) = &mut mic_encoder {
        e.stream = Stream::Microphone;
    }
    let mut desktop = HashMap::new();
    let mut apps: HashMap<u32, (String, Endpoint)> = HashMap::new();
    let names = settings.application_names();
    let mut app_encoders = Vec::new();
    if settings.desktop_audio && settings.separate_audio_tracks && settings.application_audio() {
        for (i, name) in names.iter().enumerate() {
            let mut e = AacEncoder::new()?;
            e.stream = Stream::Application(i as u16);
            app_encoders.push((name.clone(), e));
        }
    }
    let mut failed = HashSet::new();
    let mut mic = None;
    let mut processor = crate::microphone::MicrophoneProcessor::new(&settings);
    let mut retry = Instant::now() - Duration::from_secs(3);
    let mut position = 0u64;
    while !signal.load(Ordering::Relaxed) {
        if retry.elapsed() >= Duration::from_secs(2) {
            retry = Instant::now();
            if settings.desktop_audio && settings.application_audio() {
                let selected = super::process::roots(&super::process::processes(), &names);
                apps.retain(|pid, _| selected.iter().any(|a| a.pid == *pid));
                for app in selected {
                    if let std::collections::hash_map::Entry::Vacant(entry) = apps.entry(app.pid) {
                        match Endpoint::open_process(app.pid) {
                            Ok(endpoint) => {
                                entry.insert((app.executable, endpoint));
                            }
                            Err(e) => {
                                let _ = warnings.try_send(format!(
                                    "Application audio unavailable for {}: {e:#}",
                                    app.executable
                                ));
                            }
                        }
                    }
                }
            }
            if settings.desktop_audio && !settings.application_audio() {
                match enumerate_devices(false) {
                    Ok(mut devices) => {
                        if let Some(ids) = &settings.output_devices
                            && settings.audio_mode()
                                == crate::config::AudioCaptureMode::SelectedDevices
                        {
                            devices.retain(|device| ids.contains(&device.id));
                        }
                        let errors =
                            refresh_outputs(&mut desktop, &devices, |id| Endpoint::open(false, id));
                        let currently_failed: HashSet<_> =
                            errors.iter().map(|(id, _)| id.clone()).collect();
                        for (id, error) in errors {
                            if !failed.contains(&id) {
                                let _ = warnings.try_send(error);
                            }
                        }
                        if devices.is_empty() && !failed.contains("no-output") {
                            let _ = warnings.try_send("No PC playback device is available; reconnect a speaker or headset. Retrying automatically.".into());
                        }
                        failed = currently_failed;
                        if devices.is_empty() {
                            failed.insert("no-output".into());
                        }
                    }
                    Err(error) => {
                        if failed.insert("enumeration".into()) {
                            let _ = warnings.try_send(format!(
                                "Could not find PC audio devices; retrying: {error:#}"
                            ));
                        }
                    }
                }
            }
            if settings.microphone && mic.is_none() {
                match Endpoint::open(true, &settings.input_device) {
                    Ok(endpoint) => {
                        mic = Some(endpoint);
                        processor = crate::microphone::MicrophoneProcessor::new(&settings);
                    }
                    Err(error) => {
                        let _ = warnings
                            .try_send(format!("Microphone unavailable; retrying: {error:#}"));
                    }
                }
            }
        }
        apps.retain(|_, (_, endpoint)| endpoint.read(&clock).is_ok());
        desktop.retain(|_, endpoint| match endpoint.read(&clock) {
            Ok(()) => true,
            Err(error) => {
                let _ =
                    warnings.try_send(format!("PC audio device changed; reconnecting: {error}"));
                false
            }
        });
        if let Some(endpoint) = &mut mic
            && let Err(error) = endpoint.read(&clock)
        {
            let _ = warnings.try_send(format!("Microphone changed; reconnecting: {error}"));
            mic = None;
        }
        let available = clock.now() * 48_000 / TICKS;
        // 80 ms of bounded audio latency allows both devices to
        // deliver packets before mixing, independent of video.
        let ready = available.saturating_sub(3840);
        if ready.saturating_sub(position) > 96_000 {
            anyhow::bail!("Audio processing fell more than two seconds behind. Restart capture.");
        }
        while position + 1024 <= ready {
            let mut mix = [[0.0f32; 2]; 1024];
            for endpoint in desktop.values_mut() {
                endpoint.mix(position as i64, &mut mix, settings.desktop_volume, None);
            }
            if settings.application_audio() {
                for name in &names {
                    let mut app_mix = [[0.0f32; 2]; 1024];
                    for (executable, endpoint) in apps.values_mut() {
                        if executable.eq_ignore_ascii_case(name) {
                            endpoint.mix(
                                position as i64,
                                &mut app_mix,
                                settings.desktop_volume,
                                None,
                            );
                        }
                    }
                    for (out, sample) in mix.iter_mut().zip(app_mix) {
                        out[0] += sample[0];
                        out[1] += sample[1];
                    }
                    if let Some((_, encoder)) = app_encoders
                        .iter_mut()
                        .find(|(n, _)| n.eq_ignore_ascii_case(name))
                    {
                        encoder.encode(&app_mix, position * TICKS / 48_000, &budget, &packets)?;
                    }
                }
            }
            if let Some(e) = &mut pc_encoder {
                e.encode(&mix, position * TICKS / 48_000, &budget, &packets)?;
            }
            let mut input = [[0.0f32; 2]; 1024];
            if let Some(e) = &mut mic {
                e.mix(
                    position as i64 + processor.lookahead(),
                    &mut input,
                    1.0,
                    settings.mic_mono.then_some(settings.mic_channel),
                );
                processor.process(&mut input, &settings, ptt_held(&settings));
                for (out, sample) in mix.iter_mut().zip(input) {
                    out[0] += sample[0];
                    out[1] += sample[1];
                }
            }
            if let Some(e) = &mut mic_encoder {
                e.encode(&input, position * TICKS / 48_000, &budget, &packets)?;
            }
            encoder.encode(&mix, position * TICKS / 48_000, &budget, &packets)?;
            position += 1024;
        }
        std::thread::sleep(Duration::from_millis(4));
    }
    drop(encoder);
    drop(desktop);
    drop(mic);
    Ok(())
}
