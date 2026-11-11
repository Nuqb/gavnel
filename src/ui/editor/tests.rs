use super::*;
use crate::media::{Aspect, AudioTrack, ClipSegment, MediaInfo};
use std::collections::HashMap;
struct Harness {
    ctx: egui::Context,
    editor: Editor,
    service: MediaService,
    textures: HashMap<egui::TextureId, egui::ColorImage>,
    size: Vec2,
    text_input: Option<String>,
}

#[test]
fn timeline_keeps_partial_frames_and_finishes_without_reloading_textures() {
    let ctx = egui::Context::default();
    let mut editor = Editor::new(PathBuf::from("Clips"));
    let path = PathBuf::from("example.mp4");
    let image = egui::ColorImage::filled([16, 9], Color32::RED);
    editor.strip_pending = Some((42, path.clone()));
    editor.receive_filmstrip(&ctx, 42, Ok(vec![image.clone()]));
    assert!(editor.strip_pending.is_some());
    let first = editor.filmstrips[&path][0].id();
    editor.receive_filmstrip(&ctx, 42, Ok(vec![image; 8]));
    assert!(editor.strip_pending.is_none());
    assert_eq!(editor.filmstrips[&path].len(), 8);
    assert_eq!(editor.filmstrips[&path][0].id(), first);

    editor.strip_pending = Some((43, path.clone()));
    editor.receive_filmstrip(&ctx, 43, Err("Unreadable video".into()));
    assert!(editor.strip_pending.is_none());
    assert_eq!(editor.filmstrips[&path].len(), 8);
    assert_eq!(editor.error, "Unreadable video");
}
impl Harness {
    fn new(size: Vec2, tab: Tab) -> Self {
        let ctx = egui::Context::default();
        super::super::theme::apply(&ctx);
        let mut editor = Editor::new(PathBuf::from("Clips"));
        editor.loading = false;
        editor.tab = tab;
        let mut segment = ClipSegment::new(MediaInfo {
            path: PathBuf::from("example.mp4"),
            duration: 60.0,
            width: 1920,
            height: 1080,
            fps: 60.0,
            audio: vec![
                AudioTrack {
                    index: 2,
                    label: "Game".into(),
                },
                AudioTrack {
                    index: 3,
                    label: "Microphone".into(),
                },
            ],
            markers: vec![(12.0, "Kill".into()), (35.0, "Bookmark".into())],
        });
        segment.start = 5.0;
        segment.end = 45.0;
        if tab == Tab::Crop {
            segment.crop.aspect = Aspect::Vertical;
        }
        editor.project = Some(EditProject::new(vec![segment]));
        editor.cursor = 5.0;
        editor.last_frame = Some((PathBuf::from("example.mp4"), 5000));
        let mut image = egui::ColorImage::filled([640, 360], Color32::from_rgb(34, 54, 72));
        for y in 0..360 {
            for x in 0..640 {
                let c = if (x / 50 + y / 50) % 2 == 0 {
                    Color32::from_rgb(44, 70, 88)
                } else {
                    Color32::from_rgb(32, 51, 69)
                };
                image.pixels_mut()[y * 640 + x] = c;
            }
        }
        let texture = ctx.load_texture("test-preview", image, egui::TextureOptions::LINEAR);
        editor.texture = Some(texture.clone());
        editor
            .filmstrips
            .insert(PathBuf::from("example.mp4"), vec![texture; 8]);
        Self {
            ctx,
            editor,
            service: MediaService::new(),
            textures: HashMap::new(),
            size,
            text_input: None,
        }
    }
    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        let mut output = self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
                events,
                ..Default::default()
            },
            |root| {
                egui::Panel::left("rail")
                    .exact_size(66.0)
                    .show(root, |_| {});
                egui::Panel::top("toolbar")
                    .exact_size(66.0)
                    .show(root, |ui| {
                        ui.label("Gavnel");
                    });
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(BG).inner_margin(28.0))
                    .show(root, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Editor").size(24.0).strong());
                            buttons::enabled(ui, buttons::Icon::Undo, "Undo", false);
                            buttons::enabled(ui, buttons::Icon::Redo, "Redo", false);
                            buttons::enabled(ui, buttons::Icon::SaveVideo, "Save video", true);
                        });
                        if let Some(text) = &mut self.text_input {
                            ui.text_edit_singleline(text).request_focus();
                        }
                        self.editor.body(ui, &self.service, &mut None);
                    });
                if self.editor.save_open {
                    self.editor.save_modal(&self.ctx);
                }
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
}
fn icon_rect(
    output: &egui::FullOutput,
    ctx: &egui::Context,
    name: &'static str,
    svg: &[u8],
) -> egui::Rect {
    fn find(shape: &egui::Shape, id: egui::TextureId) -> Option<egui::Rect> {
        match shape {
            egui::Shape::Mesh(mesh) if mesh.texture_id == id => Some(mesh.calc_bounds()),
            egui::Shape::Rect(rect) if rect.fill_texture_id() == id => Some(rect.rect),
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|s| find(s, id)),
            _ => None,
        }
    }
    let id = crate::icons::texture(ctx, name, svg, 32).id();
    output
        .shapes
        .iter()
        .find_map(|shape| find(&shape.shape, id))
        .unwrap_or_else(|| panic!("Missing icon {name}"))
}
fn editor_icon(output: &egui::FullOutput, ctx: &egui::Context, name: &'static str) -> egui::Rect {
    let svg: &[u8] = match name {
        "editor-trim" => include_bytes!("../../../assets/icons/editor-trim.svg"),
        "editor-crop" => include_bytes!("../../../assets/icons/editor-crop.svg"),
        "editor-audio" => include_bytes!("../../../assets/icons/volume.svg"),
        "editor-play" => include_bytes!("../../../assets/icons/play.svg"),
        "editor-reset" => include_bytes!("../../../assets/icons/editor-reset.svg"),
        "editor-center" => include_bytes!("../../../assets/icons/editor-center.svg"),
        _ => panic!("unknown icon"),
    };
    icon_rect(output, ctx, name, svg)
}
#[test]
fn editor_controls_fit_and_crop_and_audio_are_reachable() {
    for size in [Vec2::new(1000.0, 650.0), Vec2::new(1280.0, 820.0)] {
        for tab in [Tab::Trim, Tab::Crop, Tab::Audio] {
            let mut h = Harness::new(size, tab);
            h.frame(vec![]);
            let output = h.frame(vec![]);
            for label in ["editor-trim", "editor-crop", "editor-audio", "editor-play"] {
                let rect = editor_icon(&output, &h.ctx, label);
                assert!(
                    rect.left() >= 0.0 && rect.right() <= size.x && rect.bottom() <= size.y,
                    "{label}: {rect:?}"
                );
            }
            let active = match tab {
                Tab::Trim | Tab::Audio => "editor-reset",
                Tab::Crop => "editor-center",
            };
            let _ = editor_icon(&output, &h.ctx, active);
        }
    }
}
#[test]
fn undo_redo_and_bookmark_selection_preserve_original() {
    let mut h = Harness::new(Vec2::new(1000.0, 650.0), Tab::Trim);
    let original = h.editor.project.clone().unwrap();
    h.editor.project.as_mut().unwrap().segments[0].bookmark(3.0);
    h.editor.record_edit(original.clone(), false);
    assert_eq!(h.editor.project.as_ref().unwrap().segments[0].end, 8.0);
    h.editor.undo();
    assert_eq!(h.editor.project.as_ref(), Some(&original));
    h.editor.redo();
    assert_eq!(h.editor.project.as_ref().unwrap().segments[0].start, 0.0);
    assert_eq!(original.segments[0].start, 5.0);
}
#[test]
#[ignore = "exports editor screenshots without opening an app or accessing capture devices"]
fn export_editor_previews() {
    std::fs::create_dir_all("work/editor-previews").unwrap();
    let sample = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("work/editor-smoke/source.mp4");
    let demo = if sample.is_file() {
        (|| {
            let frame = crate::media::frame(&sample, 1.0).ok()?;
            let strip = crate::media::filmstrip(&sample).ok()?;
            Some((frame, strip))
        })()
    } else {
        None
    };
    for (tab, name) in [
        (Tab::Trim, "trim"),
        (Tab::Crop, "crop"),
        (Tab::Audio, "audio"),
    ] {
        let mut h = Harness::new(Vec2::new(1000.0, 650.0), tab);
        if let Some((frame, strip)) = &demo {
            h.editor.texture = Some(h.ctx.load_texture(
                "sample-frame",
                frame.clone(),
                egui::TextureOptions::LINEAR,
            ));
            h.editor.filmstrips.insert(
                PathBuf::from("example.mp4"),
                strip
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        h.ctx.load_texture(
                            format!("sample-strip-{i}"),
                            f.clone(),
                            egui::TextureOptions::LINEAR,
                        )
                    })
                    .collect(),
            );
        }

        h.frame(vec![]);
        let output = h.frame(vec![]);
        let pixels = super::super::test_render::raster(&h.ctx, h.size, &h.textures, output);
        let file = std::fs::File::create(format!("work/editor-previews/{name}.png")).unwrap();
        let mut encoder = png::Encoder::new(file, 1000, 650);
        encoder.set_color(png::ColorType::Rgb);
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&pixels.into_iter().flatten().collect::<Vec<_>>())
            .unwrap();
    }
}

#[test]
fn scaled_and_rtl_editor_controls_stay_inside_the_window() {
    for language in [
        i18n::Language::EnglishUs,
        i18n::Language::Arabic,
        i18n::Language::Hebrew,
    ] {
        i18n::with_language(language, || {
            for scale in [1.0, 1.5, 2.0] {
                let mut h = Harness::new(Vec2::new(1000.0, 650.0), Tab::Audio);
                super::super::fonts::ensure_language(&h.ctx, language.code());
                h.ctx.set_pixels_per_point(scale);
                h.frame(vec![]);
                let output = h.frame(vec![]);
                for label in ["editor-audio", "editor-reset", "editor-play"] {
                    let rect = editor_icon(&output, &h.ctx, label);
                    assert!(
                        rect.right() <= 1000.0 && rect.bottom() <= 650.0,
                        "{language:?} at {scale}: {label} {rect:?}"
                    );
                }
            }
        });
    }
}

#[test]
fn play_is_available_before_a_proxy_and_space_requests_playback() {
    let mut h = Harness::new(Vec2::new(1000.0, 650.0), Tab::Trim);
    h.frame(vec![]);
    let output = h.frame(vec![]);
    let _ = editor_icon(&output, &h.ctx, "editor-play");
    assert!(h.editor.preview_job.is_none());
    h.frame(vec![egui::Event::Key {
        key: egui::Key::Space,
        physical_key: Some(egui::Key::Space),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert!(h.editor.play_requested);
    h.editor.play_requested = false;
    h.frame(vec![egui::Event::Key {
        key: egui::Key::Space,
        physical_key: Some(egui::Key::Space),
        pressed: true,
        repeat: true,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert!(!h.editor.play_requested);
}
#[test]
fn dragging_a_crop_border_switches_to_custom_and_supports_undo() {
    let mut h = Harness::new(Vec2::new(1000.0, 650.0), Tab::Crop);
    h.frame(vec![]);
    let output = h.frame(vec![]);
    let rect = output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Rect(r)
                if r.stroke.width == 1.5 && r.stroke.color == TEXT && r.rect.height() > 80.0 =>
            {
                Some(r.rect)
            }
            _ => None,
        })
        .expect("Crop border should be drawn");
    let initial = h.editor.project.as_ref().unwrap().segments[0].crop.clone();
    let origin = rect.right_center();
    let target = origin - egui::vec2(40.0, 0.0);
    h.frame(vec![egui::Event::PointerMoved(origin)]);
    h.frame(vec![egui::Event::PointerButton {
        pos: origin,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    }]);
    h.frame(vec![egui::Event::PointerMoved(target)]);
    h.frame(vec![egui::Event::PointerButton {
        pos: target,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    let segment = &h.editor.project.as_ref().unwrap().segments[0];
    assert_eq!(segment.crop.aspect, Aspect::Custom);
    assert!(segment.crop.pixels(&segment.info).2 < initial.pixels(&segment.info).2);
    h.editor.undo();
    assert_eq!(h.editor.project.as_ref().unwrap().segments[0].crop, initial);
}

#[test]
fn clicking_play_requests_playback_and_typing_space_does_not() {
    let mut h = Harness::new(Vec2::new(1000.0, 650.0), Tab::Trim);
    h.frame(vec![]);
    let output = h.frame(vec![]);
    let pos = editor_icon(&output, &h.ctx, "editor-play").center();
    h.frame(vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
    ]);
    h.frame(vec![egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert!(h.editor.play_requested);
    h.editor.play_requested = false;
    h.text_input = Some(String::new());
    h.frame(vec![]);
    h.frame(vec![]);
    h.frame(vec![
        egui::Event::Key {
            key: egui::Key::Space,
            physical_key: Some(egui::Key::Space),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::Text(" ".into()),
    ]);
    assert!(!h.editor.play_requested);
    assert_eq!(h.text_input.as_deref(), Some(" "));
}

fn save_texts(shape: &egui::Shape, labels: &mut Vec<(String, egui::Rect)>) {
    match shape {
        egui::Shape::Text(text) => {
            labels.push((text.galley.text().into(), text.visual_bounding_rect()))
        }
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                save_texts(shape, labels);
            }
        }
        _ => {}
    }
}

fn save_labels(output: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
    let mut labels = vec![];
    for shape in &output.shapes {
        save_texts(&shape.shape, &mut labels);
    }
    labels
}

#[test]
fn save_modal_is_centered_with_inline_actions_and_editable_settings() {
    let mut h = Harness::new(Vec2::new(1000.0, 650.0), Tab::Trim);
    h.editor.save_open = true;
    for _ in 0..24 {
        h.frame(vec![]);
    }
    let output = h.frame(vec![]);
    let labels = save_labels(&output);
    let find = |name: &str| labels.iter().find(|(text, _)| text == name).unwrap().1;
    assert!(
        (find("Save current video").center().y - find("Save as new video").center().y).abs() < 1.0
    );
    assert!(labels.iter().any(|(text, _)| text == "1920 \u{00d7} 1080"));
    assert!(
        !labels
            .iter()
            .any(|(text, _)| text.contains("project") || text == "720p")
    );
    fn panel_rect(shape: &egui::Shape) -> Option<egui::Rect> {
        match shape {
            egui::Shape::Rect(rect) if rect.fill == PANEL && rect.rect.width() > 400.0 => {
                Some(rect.rect)
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(panel_rect),
            _ => None,
        }
    }
    let panel = output
        .shapes
        .iter()
        .find_map(|shape| panel_rect(&shape.shape))
        .expect("modal panel");
    assert!(
        panel.center().distance(egui::pos2(500.0, 325.0)) < 2.0,
        "{panel:?}"
    );
    assert!(labels.iter().any(|(text, _)| text == "\u{2248} 61.6 MB"));
    assert!(
        !labels
            .iter()
            .any(|(text, _)| text == "No limit" || text == "Edit")
    );
    let close = icon_rect(
        &output,
        &h.ctx,
        "save-close",
        include_bytes!("../../../assets/icons/close.svg"),
    );
    assert!((close.center().y - find("Save video").center().y).abs() < 3.0);
    assert!(close.right() > panel.right() - 60.0);
    let edit = icon_rect(
        &output,
        &h.ctx,
        "save-edit",
        include_bytes!("../../../assets/icons/edit.svg"),
    )
    .center();
    h.frame(vec![
        egui::Event::PointerMoved(edit),
        egui::Event::PointerButton {
            pos: edit,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
    ]);
    h.frame(vec![egui::Event::PointerButton {
        pos: edit,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    let output = h.frame(vec![]);
    assert!(h.editor.save_editing);
    assert!(save_labels(&output).iter().any(|(text, _)| text == "720p"));
}

#[test]
#[ignore = "renders save modal images for visual review"]
fn export_save_modal_previews() {
    std::fs::create_dir_all("work/editor-previews").unwrap();
    for editing in [false, true] {
        let mut h = Harness::new(Vec2::new(1000.0, 650.0), Tab::Trim);
        h.editor.save_open = true;
        h.editor.save_editing = editing;
        for _ in 0..24 {
            h.frame(vec![]);
        }
        let output = h.frame(vec![]);
        let pixels = super::super::test_render::raster(&h.ctx, h.size, &h.textures, output);
        let file =
            std::fs::File::create(format!("work/editor-previews/save-{editing}.png")).unwrap();
        let mut encoder = png::Encoder::new(file, 1000, 650);
        encoder.set_color(png::ColorType::Rgb);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels.into_iter().flatten().collect::<Vec<_>>())
            .unwrap();
    }
}
