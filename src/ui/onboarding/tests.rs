use super::*;
use egui::Rect;
use std::collections::HashMap;

struct Harness {
    ctx: egui::Context,
    wizard: Wizard,
    textures: HashMap<egui::TextureId, egui::ColorImage>,
    size: Vec2,
    time: f64,
    action: Option<Action>,
}
impl Harness {
    fn new(size: Vec2) -> Self {
        let ctx = egui::Context::default();
        super::super::theme::apply(&ctx);
        Self {
            ctx,
            wizard: Wizard::new(Settings::default(), false),
            textures: HashMap::new(),
            size,
            time: 0.0,
            action: None,
        }
    }

    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        self.time += 1.0 / 30.0;
        let mut output = self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, self.size)),
                time: Some(self.time),
                events,
                ..Default::default()
            },
            |ctx| {
                self.action = self.wizard.show(
                    ctx,
                    Devices {
                        monitors: &[MonitorInfo {
                            index: 1,
                            label: "Display 1 · 1920 × 1080".into(),
                        }],
                        windows: &[WindowInfo {
                            handle: 1,
                            title: "Example game".into(),
                            executable: "example.exe".into(),
                        }],
                        inputs: &[AudioDevice {
                            id: "test-mic".into(),
                            label: "Example microphone".into(),
                        }],
                    },
                );
            },
        );
        for (id, deltas) in &output.textures_delta.set {
            for delta in deltas {
                let egui::ImageData::Color(image) = &delta.image;
                if let Some([x, y]) = delta.pos {
                    let texture = self.textures.get_mut(id).unwrap();
                    for row in 0..image.resolution()[1] {
                        let start = (y + row) * texture.resolution()[0] + x;
                        texture.pixels_mut()[start..start + image.resolution()[0]].copy_from_slice(
                            &image.pixels()
                                [row * image.resolution()[0]..(row + 1) * image.resolution()[0]],
                        );
                    }
                } else {
                    self.textures.insert(*id, image.as_ref().clone());
                }
            }
        }

        output.textures_delta.clear();
        output
    }

    fn settled(&mut self) -> egui::FullOutput {
        for _ in 0..15 {
            self.frame(Vec::new());
        }
        self.frame(Vec::new())
    }

    fn click(&mut self, position: egui::Pos2) {
        for pressed in [true, false] {
            self.frame(vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ]);
        }
    }
}

fn text_rect(output: &egui::FullOutput, label: &str) -> Rect {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(Rect::from_min_size(text.pos, text.galley.size()))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("Missing label: {label}"))
}

#[test]
fn every_step_keeps_navigation_visible_at_minimum_window_size_and_skip_works() {
    for size in [Vec2::new(1000.0, 650.0), Vec2::new(1200.0, 780.0)] {
        let mut harness = Harness::new(size);
        for step in Step::ALL {
            harness.wizard.go_to(step);
            let output = harness.settled();
            let label = if step == Step::Welcome {
                "Let’s set up"
            } else if step == Step::Ready {
                "Finish setup"
            } else {
                "Continue"
            };
            let button = text_rect(&output, label);
            assert!(
                button.bottom() < size.y && button.right() < size.x,
                "{step:?}: {button:?} exceeds {size:?}"
            );
            let skip = text_rect(&output, "Skip setup").center();
            harness.click(skip);
            assert_eq!(harness.action, Some(Action::Skip));
        }
    }
}

#[test]
fn every_localized_step_fits_and_can_be_skipped_at_minimum_size() {
    let mut harness = Harness::new(Vec2::new(1000.0, 650.0));
    harness.wizard.draft.microphone = true;
    for language in i18n::Language::ALL
        .into_iter()
        .filter(|language| language.is_selectable())
    {
        harness.wizard.draft.language = language;
        for step in Step::ALL {
            harness.wizard.go_to(step);
            let output = harness.settled();
            if step != Step::Welcome {
                let heading = i18n::with_language(language, || step.label().to_owned());
                text_rect(&output, &heading);
            }
            let source = match step {
                Step::Welcome => "Let’s set up",
                Step::Ready => "Finish setup",
                _ => "Continue",
            };
            let label = i18n::with_language(language, || tr(source).to_owned());
            let rect = text_rect(&output, &label);
            assert!(
                rect.bottom() < harness.size.y && rect.right() < harness.size.x,
                "{language:?} {step:?}: {rect:?}"
            );
            let skip = i18n::with_language(language, || tr("Skip setup").to_owned());
            harness.click(text_rect(&output, &skip).center());
            assert_eq!(harness.action, Some(Action::Skip));
        }
    }
}

#[test]
fn welcome_language_picker_changes_the_draft_without_saving() {
    let mut harness = Harness::new(Vec2::new(1000.0, 650.0));
    let output = harness.settled();
    harness.click(text_rect(&output, "English (United States)").center());
    let output = harness.settled();
    harness.click(text_rect(&output, "Español (España)").center());
    assert_eq!(harness.wizard.draft.language, i18n::Language::SpanishSpain);
    let output = harness.settled();
    text_rect(&output, "Te damos la bienvenida a Gavnel.");
    assert!(!harness.wizard.draft.onboarding_completed);
    assert!(harness.action.is_none());
}

#[test]
fn supported_languages_have_glyphs_in_the_actual_app_font() {
    let mut harness = Harness::new(Vec2::new(1000.0, 650.0));
    harness.settled();
    harness.ctx.fonts_mut(|fonts| {
        let font = egui::FontId::proportional(14.0);
        for language in i18n::Language::ALL
            .into_iter()
            .filter(|language| language.is_selectable())
        {
            i18n::with_language(language, || {
                let source: HashMap<String, String> =
                    serde_json::from_str(include_str!("../../../assets/locales/en-US.json"))
                        .unwrap();
                for text in std::iter::once(language.name()).chain(source.keys().map(|key| tr(key)))
                {
                    assert!(fonts.has_glyphs(&font, text), "{language:?}: {text}");
                }
            });
        }
    });
}

#[test]
#[ignore = "exports localized onboarding without devices or preference writes"]
fn export_localized_onboarding_previews() {
    std::fs::create_dir_all("work/languages").unwrap();
    let mut harness = Harness::new(Vec2::new(1000.0, 650.0));
    for language in [
        i18n::Language::Arabic,
        i18n::Language::Hebrew,
        i18n::Language::Hindi,
        i18n::Language::Bengali,
        i18n::Language::ChineseTraditional,
        i18n::Language::Japanese,
        i18n::Language::Thai,
        i18n::Language::Amharic,
    ] {
        harness.wizard.draft.language = language;
        for step in [Step::Welcome, Step::Hotkeys, Step::Preferences, Step::Ready] {
            harness.wizard.go_to(step);
            let output = harness.settled();
            let pixels = super::super::test_render::raster(
                &harness.ctx,
                harness.size,
                &harness.textures,
                output,
            );
            let path = format!("work/languages/{language:?}-{}.bmp", step.index());
            super::super::test_render::write_bitmap(
                &path,
                harness.size.x as usize,
                harness.size.y as usize,
                &pixels,
            );
        }
    }
}

#[test]
fn simplified_capture_choices_and_smooth_hd_preset_update_the_draft() {
    let mut harness = Harness::new(Vec2::new(1000.0, 650.0));
    harness.wizard.go_to(Step::Capture);
    let output = harness.settled();
    harness.click(text_rect(&output, "Whole screen").center());
    assert_eq!(harness.wizard.draft.mode, CaptureMode::Monitor);
    let output = harness.settled();
    harness.click(text_rect(&output, "Game / window").center());
    assert_eq!(harness.wizard.draft.mode, CaptureMode::Automatic);

    harness.wizard.go_to(Step::Quality);
    let output = harness.settled();
    harness.click(text_rect(&output, "Smooth HD\n720p · 60 FPS · 15 Mbps").center());
    let candidate = harness.wizard.candidate().unwrap();
    assert_eq!(
        (candidate.height, candidate.fps, candidate.bitrate_mbps),
        (720, 60, 15)
    );
}

#[test]
fn audio_and_expanded_quality_fit_above_navigation() {
    let mut harness = Harness::new(Vec2::new(1000.0, 650.0));
    harness.wizard.draft.microphone = true;
    harness.wizard.go_to(Step::Audio);
    let output = harness.settled();
    assert!(text_rect(&output, "Test microphone").bottom() < text_rect(&output, "Continue").top());

    harness.wizard.go_to(Step::Quality);
    let output = harness.settled();
    harness.click(text_rect(&output, "Customize quality").center());
    let output = harness.settled();
    assert!(text_rect(&output, "Continue").bottom() < harness.size.y);
}

#[test]
fn hotkey_capture_rejects_duplicates_and_escape_cancels_without_changes() {
    let mut harness = Harness::new(Vec2::new(1200.0, 780.0));
    harness.wizard.listening = Some(1);
    let event = |key| egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    };
    harness.frame(vec![event(egui::Key::F4)]);
    assert_eq!(harness.wizard.draft.hotkey, "F8");
    assert!(!harness.wizard.error.is_empty());
    harness.wizard.listening = Some(1);
    harness.frame(vec![event(egui::Key::F10)]);
    assert_eq!(harness.wizard.draft.hotkey, "F10");
    harness.wizard.listening = Some(1);
    harness.frame(vec![event(egui::Key::Escape)]);
    assert_eq!(harness.wizard.draft.hotkey, "F10");
    assert!(harness.wizard.listening.is_none());
}

#[test]
fn replay_duration_updates_primary_binding_and_survives_settings_serialization() {
    let mut wizard = Wizard::new(Settings::default(), false);
    wizard.draft.edit_capture_bindings();
    wizard.set_replay_seconds(120);
    let settings = wizard.candidate().unwrap();
    let restored: Settings =
        serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
    assert_eq!(restored.replay_seconds, 120);
    assert_eq!(restored.capture_bindings()[0].clip_seconds, 120);
}

#[test]
fn continue_back_and_finish_traverse_the_draft_without_committing() {
    let mut harness = Harness::new(Vec2::new(1200.0, 780.0));
    for expected in Step::ALL.into_iter().skip(1) {
        let output = harness.settled();
        let label = if harness.wizard.step == Step::Welcome {
            "Let’s set up"
        } else {
            "Continue"
        };
        harness.click(text_rect(&output, label).center());
        assert_eq!(harness.wizard.step, expected);
        assert!(harness.action.is_none());
    }
    let output = harness.settled();
    harness.click(text_rect(&output, "Back").center());
    assert_eq!(harness.wizard.step, Step::Preferences);
    let output = harness.settled();
    harness.click(text_rect(&output, "Continue").center());
    let output = harness.settled();
    harness.click(text_rect(&output, "Finish setup").center());
    assert_eq!(harness.action, Some(Action::Finish));
    assert!(!harness.wizard.draft.onboarding_completed);
}

#[test]
#[ignore = "exports all onboarding steps without opening a window or accessing devices"]
fn export_onboarding_previews() {
    std::fs::create_dir_all("work/onboarding").unwrap();
    let mut harness = Harness::new(Vec2::new(1200.0, 780.0));
    for step in Step::ALL {
        harness.wizard.go_to(step);
        let output = harness.settled();
        let pixels = super::super::test_render::raster(
            &harness.ctx,
            harness.size,
            &harness.textures,
            output,
        );
        super::super::test_render::write_bitmap(
            &format!("work/onboarding/{:02}.bmp", step.index()),
            1200,
            780,
            &pixels,
        );
    }
}

#[test]
fn draft_changes_do_not_modify_original_and_finish_marks_setup_complete() {
    let original = Settings::default();
    let mut wizard = Wizard::new(original.clone(), false);
    wizard.draft.height = 720;
    wizard.draft.microphone = true;
    let finished = wizard.candidate().unwrap();
    assert_eq!(original.height, 1080);
    assert!(!original.microphone && !original.onboarding_completed);
    assert!(finished.onboarding_completed && finished.microphone);
    assert_eq!(finished.capture_enabled, Some(false));
    assert_eq!(finished.height, 720);
}

#[test]
fn skipping_remembers_language_but_discards_other_draft_edits() {
    let original = Settings::default();
    let mut wizard = Wizard::new(original.clone(), false);
    wizard.draft.language = i18n::Language::SpanishMexico;
    wizard.draft.height = 720;
    wizard.draft.microphone = true;
    wizard.folder.clear();
    let skipped = wizard.skipped_settings(&original);
    assert_eq!(
        skipped,
        Settings {
            language: i18n::Language::SpanishMexico,
            onboarding_completed: true,
            ..original
        }
    );
}

#[test]
fn window_selection_and_folder_are_validated_and_back_discards_key_listener() {
    let mut wizard = Wizard::new(Settings::default(), false);
    wizard.step = Step::Capture;
    wizard.draft.mode = CaptureMode::Window;
    wizard.advance();
    assert_eq!(wizard.step, Step::Capture);
    assert!(!wizard.error.is_empty());
    wizard.draft.window_title = "A game".into();
    wizard.advance();
    assert_eq!(wizard.step, Step::Quality);
    wizard.folder.clear();
    assert!(wizard.candidate().is_err());
    wizard.listening = Some(1);
    wizard.go_to(Step::Hotkeys);
    assert!(wizard.listening.is_none());
}

#[test]
fn finishing_never_enables_capture_in_off_mode() {
    let mut wizard = Wizard::new(
        Settings {
            recording_mode: RecordingMode::Off,
            ..Settings::default()
        },
        true,
    );
    assert_eq!(wizard.candidate().unwrap().capture_enabled, Some(false));
    wizard.draft.recording_mode = RecordingMode::Clips;
    assert_eq!(wizard.candidate().unwrap().capture_enabled, Some(true));
}

#[test]
fn right_to_left_navigation_places_back_on_the_right_and_continue_on_the_left() {
    let mut harness = Harness::new(Vec2::new(1000.0, 650.0));
    for language in [i18n::Language::Arabic, i18n::Language::Hebrew] {
        harness.wizard.draft.language = language;
        harness.wizard.go_to(Step::Quality);
        let output = harness.settled();
        let (back, next) = i18n::with_language(language, || {
            (tr("Back").to_owned(), tr("Continue").to_owned())
        });
        assert!(text_rect(&output, &back).left() > text_rect(&output, &next).right());
        harness.click(text_rect(&output, &next).center());
        assert_eq!(harness.wizard.step, Step::Hotkeys);
        assert!(!harness.wizard.draft.onboarding_completed);
    }
}

#[test]
fn language_modal_search_selects_and_dismisses_without_changing_other_preferences() {
    let mut harness = Harness::new(Vec2::new(1000.0, 650.0));
    let output = harness.settled();
    harness.click(text_rect(&output, "English (United States)").center());
    harness.settled();
    harness.frame(vec![egui::Event::Text("Hindi".into())]);
    let output = harness.settled();
    text_rect(&output, "हिन्दी");
    assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "Español (España)")));
    harness.frame(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }]);
    let output = harness.settled();
    assert_eq!(harness.wizard.draft.language, i18n::Language::EnglishUs);
    assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "Hindi · hi-IN")));
    harness.click(text_rect(&output, "English (United States)").center());
    harness.settled();
    harness.frame(vec![egui::Event::Text("hi-IN".into())]);
    let output = harness.settled();
    harness.click(text_rect(&output, "हिन्दी").center());
    assert_eq!(harness.wizard.draft.language, i18n::Language::Hindi);
    assert!(!harness.wizard.draft.onboarding_completed);
    assert!(harness.action.is_none());
    let output = harness.settled();
    harness.click(text_rect(&output, "हिन्दी").center());
    harness.settled();
    harness.click(egui::pos2(8.0, 8.0));
    let output = harness.settled();
    assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "Hindi · hi-IN")));
}

#[test]
#[ignore = "exports the language modal without opening a window or recording"]
fn export_language_modal_preview() {
    let mut harness = Harness::new(Vec2::new(1000.0, 650.0));
    let output = harness.settled();
    harness.click(text_rect(&output, "English (United States)").center());
    let output = harness.settled();
    let pixels =
        super::super::test_render::raster(&harness.ctx, harness.size, &harness.textures, output);
    super::super::test_render::write_bitmap("work/language-modal.bmp", 1000, 650, &pixels);
}
