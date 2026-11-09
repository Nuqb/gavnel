//! Local notification PCM generation.
pub(super) fn chime(volume: f32) -> Vec<u8> {
    const RATE: u32 = 48_000;
    let samples = RATE * 3 / 10;
    let bytes = samples * 2;
    let mut wave = Vec::with_capacity(44 + bytes as usize);
    wave.extend_from_slice(b"RIFF");
    wave.extend_from_slice(&(36 + bytes).to_le_bytes());
    wave.extend_from_slice(b"WAVEfmt ");
    wave.extend_from_slice(&16u32.to_le_bytes());
    wave.extend_from_slice(&1u16.to_le_bytes());
    wave.extend_from_slice(&1u16.to_le_bytes());
    wave.extend_from_slice(&RATE.to_le_bytes());
    wave.extend_from_slice(&(RATE * 2).to_le_bytes());
    wave.extend_from_slice(&2u16.to_le_bytes());
    wave.extend_from_slice(&16u16.to_le_bytes());
    wave.extend_from_slice(b"data");
    wave.extend_from_slice(&bytes.to_le_bytes());
    for i in 0..samples {
        let t = i as f32 / RATE as f32;
        let mut value = 0.0;
        for (start, frequency, duration) in [(0.0, 659.25, 0.18), (0.10, 987.77, 0.20)] {
            let local = t - start;
            if (0.0..duration).contains(&local) {
                let envelope = (local / 0.012)
                    .min((duration - local) / 0.08)
                    .clamp(0.0, 1.0);
                value +=
                    (std::f32::consts::TAU * frequency * local).sin() * envelope * envelope * 0.13;
            }
        }
        wave.extend_from_slice(&((value * volume.clamp(0.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    wave
}
