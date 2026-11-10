//! OCR runs on bounded HUD images on its own WinRT thread, never on capture.
use super::*;
use std::{collections::HashMap, time::Duration};
use windows::{
    Globalization::Language,
    Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap},
    Media::Ocr::OcrEngine,
    Storage::Streams::DataWriter,
};
#[derive(Default)]
pub struct Confirmation {
    counts: HashMap<EventKind, u8>,
    latched: HashMap<EventKind, u64>,
}
impl Confirmation {
    pub fn observe(&mut self, kind: EventKind, present: bool, ticks: u64) -> bool {
        let count = self.counts.entry(kind).or_default();
        if !present {
            *count = 0;
            self.latched.remove(&kind);
            return false;
        }
        *count = count.saturating_add(1);
        if *count < 2 || self.latched.contains_key(&kind) {
            return false;
        }
        self.latched.insert(kind, ticks);
        true
    }
}
pub(super) fn recognize(
    engine: &OcrEngine,
    image: &crate::screenshot::Screenshot,
) -> anyhow::Result<String> {
    let writer = DataWriter::new()?;
    let mut bgra = image.rgba.clone();
    for pixel in bgra.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    writer.WriteBytes(&bgra)?;
    let buffer = writer.DetachBuffer()?;
    let bitmap = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
        &buffer,
        BitmapPixelFormat::Bgra8,
        image.width as i32,
        image.height as i32,
        BitmapAlphaMode::Ignore,
    )?;
    let result = engine.RecognizeAsync(&bitmap)?.join()?;
    Ok(result.Text()?.to_string().to_lowercase())
}
pub(super) fn start(
    game: Game,
    tx: Sender<GameEvent>,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<String>>,
) -> (Sender<VisualBatch>, std::thread::JoinHandle<()>) {
    let (sender, receiver) = bounded::<VisualBatch>(1);
    let worker = std::thread::Builder::new()
        .name("gavnel-visual-events".into())
        .spawn(move || {
            unsafe {
                let _ = windows::Win32::System::WinRT::RoInitialize(
                    windows::Win32::System::WinRT::RO_INIT_MULTITHREADED,
                );
            }
            let engine = Language::CreateLanguage(&windows::core::HSTRING::from("en-US"))
                .and_then(|l| OcrEngine::TryCreateFromLanguage(&l));
            match engine {
                Ok(engine) => {
                    let mut confirmation = Confirmation::default();
                    *status.lock().unwrap() = "Experimental visual detection · English HUD".into();
                    while !stop.load(Ordering::Acquire) {
                        let Ok(batch) = receiver.recv_timeout(Duration::from_millis(100)) else {
                            continue;
                        };
                        for (rule, image, ticks) in batch {
                            match recognize(&engine, &image) {
                                Ok(text) => {
                                    let matched = rule
                                        .phrases
                                        .iter()
                                        .any(|phrase| contains_phrase(&text, phrase));
                                    if confirmation.observe(rule.kind, matched, ticks) {
                                        let _ = tx.try_send(GameEvent {
                                            game,
                                            kind: rule.kind,
                                            ticks: ticks.saturating_sub(crate::replay::TICKS / 2),
                                            source: "Experimental visual detection".into(),
                                            confidence: 0.8,
                                        });
                                    }
                                }
                                Err(e) => {
                                    *status.lock().unwrap() =
                                        format!("Visual detection unavailable: {e}");
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    *status.lock().unwrap() = format!("English Windows OCR is unavailable: {e}")
                }
            }
            unsafe {
                windows::Win32::System::WinRT::RoUninitialize();
            }
        })
        .expect("Could not start visual highlight worker");
    (sender, worker)
}
fn contains_phrase(text: &str, phrase: &str) -> bool {
    let normalize = |s: &str| {
        s.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let text = format!(" {} ", normalize(text));
    let phrase = format!(" {} ", normalize(phrase));
    text.contains(&phrase)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transient_and_latched_notifications_do_not_duplicate() {
        let mut c = Confirmation::default();
        assert!(!c.observe(EventKind::Kill, true, 0));
        assert!(c.observe(EventKind::Kill, true, 1));
        assert!(!c.observe(EventKind::Kill, true, 2));
        assert!(!c.observe(EventKind::Kill, false, 3));
        assert!(!c.observe(EventKind::Kill, true, 4));
        assert!(c.observe(EventKind::Kill, true, 5));
    }
    #[test]
    fn phrases_match_words_not_unrelated_substrings() {
        assert!(contains_phrase("#1 VICTORY ROYALE!", "victory royale"));
        assert!(!contains_phrase("undead", "dead"));
        assert!(!contains_phrase("killed by", "kill"));
    }
}

#[cfg(test)]
mod integration {
    use super::*;
    #[test]
    #[ignore = "requires Windows English OCR and the bundled editing runtime; synthetic image only"]
    fn windows_ocr_recognizes_synthetic_highlight() {
        unsafe {
            windows::Win32::System::WinRT::RoInitialize(
                windows::Win32::System::WinRT::RO_INIT_MULTITHREADED,
            )
            .unwrap();
        }
        let tools = crate::media::Tools::locate().unwrap();
        let output = tools.command(false).args(["-v","error","-f","lavfi","-i","color=c=black:size=1000x200:duration=1","-vf","drawtext=fontfile='C\\:/Windows/Fonts/segoeuib.ttf':text='VICTORY ROYALE':fontsize=70:fontcolor=white:x=30:y=60","-frames:v","1","-f","rawvideo","-pix_fmt","rgba","pipe:1"]).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let engine = OcrEngine::TryCreateFromLanguage(
            &Language::CreateLanguage(&windows::core::HSTRING::from("en-US")).unwrap(),
        )
        .unwrap();
        let text = recognize(
            &engine,
            &crate::screenshot::Screenshot {
                width: 1000,
                height: 200,
                rgba: output.stdout,
            },
        )
        .unwrap();
        assert!(
            contains_phrase(&text, "victory royale"),
            "OCR returned: {text}"
        );
        drop(engine);
        unsafe {
            windows::Win32::System::WinRT::RoUninitialize();
        }
    }
}
