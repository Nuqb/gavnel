use super::*;

struct Harness {
    ctx: egui::Context,
    time: f64,
    selected: u8,
    anchor: Rect,
    popup: Id,
    calls: usize,
    show_control: bool,
    option: Rect,
    textures: std::collections::HashMap<egui::TextureId, egui::ColorImage>,
    size: Vec2,
    delete_path: Option<std::path::PathBuf>,
    delete_action: crate::ui::delete::Action,
    other_clicked: bool,
    other_rect: Rect,
    recording_settings: Option<Settings>,
    selected_hotkey: Option<u64>,
}
impl Harness {
    fn new() -> Self {
        let ctx = egui::Context::default();
        crate::ui::theme::apply(&ctx);
        Self {
            ctx,
            time: 0.0,
            selected: 0,
            anchor: Rect::NOTHING,
            popup: Id::NULL,
            calls: 0,
            show_control: true,
            option: Rect::NOTHING,
            textures: Default::default(),
            size: Vec2::new(340.0, 260.0),
            delete_path: None,
            delete_action: crate::ui::delete::Action::None,
            other_clicked: false,
            other_rect: Rect::NOTHING,
            recording_settings: None,
            selected_hotkey: None,
        }
    }
    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        self.time += 1.0 / 60.0;
        let output = self.ctx.run(
            egui::RawInput {
                time: Some(self.time),
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, self.size)),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    if let Some(settings) = &mut self.recording_settings {
                        ui.heading("Recording");
                        ui.add_space(12.0);
                        super::super::views::section(ui, "Recording Mode", |ui| {
                            super::super::widgets::recording_modes(
                                ui,
                                &mut settings.recording_mode,
                            );
                        });
                        ui.add_space(14.0);
                        if let Some(id) = super::super::shortcuts::hotkeys_panel(ui, settings, None)
                        {
                            self.selected_hotkey = Some(id);
                        }
                        return;
                    }
                    ui.heading("Microphone");
                    ui.label("Choose an audio input");
                    if self.show_control {
                        let response = ComboBox::from_id_salt("animated-test")
                            .width(280.0)
                            .selected_text(if self.selected == 0 {
                                "Windows default"
                            } else {
                                "Scarlett microphone"
                            })
                            .show_ui(ui, |ui| {
                                self.calls += 1;
                                ui.selectable_value(&mut self.selected, 0, "Windows default");
                                self.option = ui
                                    .selectable_value(&mut self.selected, 1, "Scarlett microphone")
                                    .rect;
                                ui.label("Refresh audio devices");
                            });
                        self.anchor = response.response.rect;
                        self.popup = response.response.id.with("popup");
                    }
                    ui.add_space(135.0);
                    let response = ui
                        .button("Other control")
                        .on_hover_text("Should stay usable");
                    self.other_rect = response.rect;
                    self.other_clicked |= response.clicked();
                });
                finish(ctx);
                if let Some(path) = &self.delete_path {
                    let clip = crate::library::Clip {
                        path: path.clone(),
                        name: "Fortnite highlight".into(),
                        source: "Fortnite".into(),
                        bytes: 35 * 1024 * 1024,
                        duration: 45.0,
                        modified: std::time::SystemTime::UNIX_EPOCH,
                    };
                    self.delete_action = crate::ui::delete::show(ctx, path, Some(&clip), None);
                }
            },
        );
        for (id, delta) in &output.textures_delta.set {
            let egui::ImageData::Color(image) = &delta.image;
            if let Some([x, y]) = delta.pos {
                let texture = self.textures.get_mut(id).unwrap();
                for row in 0..image.size[1] {
                    let start = (y + row) * texture.size[0] + x;
                    texture.pixels[start..start + image.size[0]].copy_from_slice(
                        &image.pixels[row * image.size[0]..(row + 1) * image.size[0]],
                    );
                }
            } else {
                self.textures.insert(*id, image.as_ref().clone());
            }
        }
        output
    }
    fn idle(&mut self, count: usize) -> egui::FullOutput {
        let mut output = self.frame(vec![]);
        for _ in 1..count {
            output = self.frame(vec![]);
        }
        output
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
    fn state(&self) -> PopupMotion {
        self.ctx
            .data(|d| d.get_temp::<PopupMotion>(state_id(self.popup)))
            .unwrap()
    }
}

// Optional visual QA: render real egui triangles/font textures offscreen. No app window.
#[test]
#[ignore = "writes an offscreen animation contact sheet for visual QA"]
fn export_dropdown_animation_preview() {
    let mut h = Harness::new();
    let mut frames = Vec::new();
    let output = h.idle(3);
    frames.push(raster(&h, output));
    h.click(h.anchor.center());
    let output = h.idle(2);
    frames.push(raster(&h, output));
    let output = h.idle(4);
    frames.push(raster(&h, output));
    let output = h.idle(10);
    frames.push(raster(&h, output));
    h.frame(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }]);
    let output = h.idle(2);
    frames.push(raster(&h, output));
    let output = h.idle(10);
    frames.push(raster(&h, output));
    let width = 340 * 3;
    let height = 260 * 2;
    let mut sheet = vec![[0u8; 3]; width * height];
    for (index, frame) in frames.iter().enumerate() {
        for y in 0..260 {
            for x in 0..340 {
                sheet[(index / 3 * 260 + y) * width + index % 3 * 340 + x] = frame[y * 340 + x];
            }
        }
    }
    write_bitmap("work/dropdown-animation-preview.bmp", width, height, &sheet);
}

fn write_bitmap(path: &str, width: usize, height: usize, sheet: &[[u8; 3]]) {
    use std::io::Write;
    let mut file = std::fs::File::create(path).unwrap();
    let stride = (width * 3).div_ceil(4) * 4;
    file.write_all(b"BM").unwrap();
    file.write_all(&((54 + stride * height) as u32).to_le_bytes())
        .unwrap();
    file.write_all(&[0; 4]).unwrap();
    file.write_all(&54u32.to_le_bytes()).unwrap();
    file.write_all(&40u32.to_le_bytes()).unwrap();
    file.write_all(&(width as i32).to_le_bytes()).unwrap();
    file.write_all(&(height as i32).to_le_bytes()).unwrap();
    file.write_all(&1u16.to_le_bytes()).unwrap();
    file.write_all(&24u16.to_le_bytes()).unwrap();
    file.write_all(&[0; 24]).unwrap();
    for y in (0..height).rev() {
        for x in 0..width {
            let p = sheet[y * width + x];
            file.write_all(&[p[2], p[1], p[0]]).unwrap();
        }
        file.write_all(&vec![0; stride - width * 3]).unwrap();
    }
}

fn delete_harness() -> Harness {
    let mut h = Harness::new();
    h.size = Vec2::new(960.0, 640.0);
    h.show_control = false;
    h.delete_path = Some(std::path::PathBuf::from(
        "Fortnite_2026-11-08_18-23-30_124.mp4",
    ));
    h
}

fn text_position(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(text) if text.galley.text() == label => {
            Some(text.pos + text.galley.size() * 0.5)
        }
        egui::Shape::Vec(shapes) => shapes.iter().find_map(|s| text_position(s, label)),
        _ => None,
    }
}

#[test]
fn delete_modal_blocks_background_supports_cancel_and_requires_explicit_delete() {
    use crate::ui::delete::Action;
    let mut h = delete_harness();
    let output = h.idle(10);
    assert_eq!(h.delete_action, Action::None);
    assert!(
        h.ctx.memory(|memory| memory.focused()).is_none(),
        "Opening the modal must not focus a button automatically"
    );
    let delete = output
        .shapes
        .iter()
        .find_map(|s| text_position(&s.shape, "Delete clip"))
        .unwrap();
    h.click(delete);
    assert_eq!(h.delete_action, Action::Delete);
    let mut h = delete_harness();
    h.idle(10);
    h.click(h.other_rect.center());
    assert_eq!(h.delete_action, Action::Cancel);
    assert!(
        !h.other_clicked,
        "Modal click passed through to the background"
    );
    let mut h = delete_harness();
    h.idle(10);
    h.frame(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }]);
    assert_eq!(h.delete_action, Action::Cancel);
}

#[test]
#[ignore = "writes an offscreen delete modal preview for visual QA"]
fn export_delete_modal_preview() {
    let mut h = delete_harness();
    let output = h.idle(12);
    write_bitmap(
        "work/delete-modal-preview.bmp",
        h.size.x as usize,
        h.size.y as usize,
        &raster(&h, output),
    );
    let output = h.idle(2);
    let delete = output
        .shapes
        .iter()
        .find_map(|s| text_position(&s.shape, "Delete clip"))
        .unwrap();
    h.frame(vec![egui::Event::PointerMoved(delete)]);
    let output = h.idle(12);
    write_bitmap(
        "work/delete-modal-hover-preview.bmp",
        h.size.x as usize,
        h.size.y as usize,
        &raster(&h, output),
    );
}

#[test]
#[ignore = "writes an offscreen recording settings preview for visual QA"]
fn export_recording_settings_preview() {
    let mut h = Harness::new();
    h.size = Vec2::new(900.0, 760.0);
    h.recording_settings = Some(Settings::default());
    let output = h.idle(12);
    write_bitmap(
        "work/recording-settings-preview.bmp",
        h.size.x as usize,
        h.size.y as usize,
        &raster(&h, output),
    );
    let mut extra = h.recording_settings.as_ref().unwrap().capture_bindings()[0].clone();
    extra.id = 5;
    extra.key = "Control+F8".into();
    h.recording_settings
        .as_mut()
        .unwrap()
        .edit_capture_bindings()
        .push(extra);
    let output = h.idle(12);
    let key = output
        .shapes
        .iter()
        .find_map(|s| text_position(&s.shape, "F8"))
        .unwrap();
    h.click(egui::pos2(h.size.x - 41.0, key.y));
    let output = h.idle(16);
    let remove = output
        .shapes
        .iter()
        .find_map(|s| text_position(&s.shape, "Remove hotkey"))
        .unwrap();
    h.frame(vec![egui::Event::PointerMoved(remove)]);
    let output = h.idle(12);
    write_bitmap(
        "work/hotkey-actions-menu-preview.bmp",
        h.size.x as usize,
        h.size.y as usize,
        &raster(&h, output),
    );
}

#[test]
fn hotkey_menu_only_offers_remove_for_an_extra_row_of_the_same_action() {
    let mut h = Harness::new();
    h.size = Vec2::new(900.0, 1000.0);
    h.recording_settings = Some(Settings::default());
    let output = h.idle(12);
    let key = output
        .shapes
        .iter()
        .find_map(|s| text_position(&s.shape, "F8"))
        .unwrap();
    h.click(egui::pos2(h.size.x - 41.0, key.y));
    let output = h.idle(16);
    assert!(
        output
            .shapes
            .iter()
            .any(|s| text_position(&s.shape, "Unset hotkey").is_some())
    );
    assert!(
        !output
            .shapes
            .iter()
            .any(|s| text_position(&s.shape, "Remove hotkey").is_some())
    );
    h.click(egui::pos2(8.0, 8.0));
    let mut extra = h.recording_settings.as_ref().unwrap().capture_bindings()[0].clone();
    extra.id = 5;
    extra.key = "Control+F8".into();
    h.recording_settings
        .as_mut()
        .unwrap()
        .edit_capture_bindings()
        .push(extra);
    h.idle(12);
    h.click(egui::pos2(h.size.x - 41.0, key.y));
    let output = h.idle(16);
    let remove = output
        .shapes
        .iter()
        .find_map(|s| text_position(&s.shape, "Remove hotkey"))
        .unwrap();
    h.click(remove);
    let bindings = h.recording_settings.as_ref().unwrap().capture_bindings();
    assert_eq!(bindings.len(), 4);
    assert!(!bindings.iter().any(|binding| binding.id == 1));
    assert!(bindings.iter().any(|binding| binding.id == 5));
}

#[test]
fn hotkey_panel_add_trigger_and_unset_controls_work() {
    use crate::capture_hotkeys::{CaptureAction, HotkeyTrigger};
    let mut h = Harness::new();
    h.size = Vec2::new(900.0, 1000.0);
    h.recording_settings = Some(Settings::default());
    let output = h.idle(12);
    let add = output
        .shapes
        .iter()
        .find_map(|s| text_position(&s.shape, "Add Hotkey"))
        .unwrap();
    h.click(add);
    let output = h.idle(12);
    let screenshot = output
        .shapes
        .iter()
        .filter_map(|s| text_position(&s.shape, "Screenshot"))
        .next_back()
        .unwrap();
    h.click(screenshot);
    let bindings = h.recording_settings.as_ref().unwrap().capture_bindings();
    assert_eq!(bindings.len(), 5);
    assert_eq!(bindings.last().unwrap().action, CaptureAction::Screenshot);
    assert_eq!(h.selected_hotkey, Some(bindings.last().unwrap().id));
    let output = h.idle(12);
    let trigger = output
        .shapes
        .iter()
        .find_map(|s| text_position(&s.shape, "Tap"))
        .unwrap();
    h.click(trigger);
    let output = h.idle(16);
    let long = output
        .shapes
        .iter()
        .find_map(|s| text_position(&s.shape, "Long press"))
        .unwrap();
    h.click(long);
    assert_eq!(
        h.recording_settings.as_ref().unwrap().capture_bindings()[0].trigger,
        HotkeyTrigger::LongPress
    );
    let output = h.idle(12);
    let key = output
        .shapes
        .iter()
        .find_map(|s| text_position(&s.shape, "F8"))
        .unwrap();
    h.click(egui::pos2(h.size.x - 41.0, key.y));
    let output = h.idle(16);
    let unset = output
        .shapes
        .iter()
        .find_map(|s| text_position(&s.shape, "Unset hotkey"))
        .unwrap();
    h.click(unset);
    assert!(
        h.recording_settings.as_ref().unwrap().capture_bindings()[0]
            .key
            .is_empty()
    );
}

fn raster(h: &Harness, output: egui::FullOutput) -> Vec<[u8; 3]> {
    let width = h.size.x as usize;
    let height = h.size.y as usize;
    let mut pixels = vec![[17, 17, 19]; width * height];
    let edge = |a: egui::Pos2, b: egui::Pos2, p: egui::Pos2| {
        (p.x - a.x) * (b.y - a.y) - (p.y - a.y) * (b.x - a.x)
    };
    for primitive in h.ctx.tessellate(output.shapes, output.pixels_per_point) {
        let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive else {
            continue;
        };
        let Some(texture) = h.textures.get(&mesh.texture_id) else {
            continue;
        };
        for indices in mesh.indices.as_chunks::<3>().0 {
            let vertices = indices.map(|i| mesh.vertices[i as usize]);
            let bounds = Rect::from_points(&vertices.map(|v| v.pos))
                .intersect(primitive.clip_rect)
                .intersect(Rect::from_min_size(egui::Pos2::ZERO, h.size));
            let area = edge(vertices[0].pos, vertices[1].pos, vertices[2].pos);
            if area.abs() < 0.00001 {
                continue;
            }
            for y in bounds.top().floor() as i32..bounds.bottom().ceil() as i32 {
                for x in bounds.left().floor() as i32..bounds.right().ceil() as i32 {
                    let p = egui::pos2(x as f32 + 0.5, y as f32 + 0.5);
                    let weights = [
                        edge(vertices[1].pos, vertices[2].pos, p) / area,
                        edge(vertices[2].pos, vertices[0].pos, p) / area,
                        edge(vertices[0].pos, vertices[1].pos, p) / area,
                    ];
                    if weights.iter().any(|v| *v < -0.00001) {
                        continue;
                    }
                    let uv = vertices
                        .iter()
                        .zip(weights)
                        .fold(Vec2::ZERO, |sum, (v, w)| sum + v.uv.to_vec2() * w);
                    let tx = (uv.x * texture.size[0] as f32)
                        .floor()
                        .clamp(0.0, (texture.size[0] - 1) as f32)
                        as usize;
                    let ty = (uv.y * texture.size[1] as f32)
                        .floor()
                        .clamp(0.0, (texture.size[1] - 1) as f32)
                        as usize;
                    let texel = texture.pixels[ty * texture.size[0] + tx].to_array();
                    let mut color = [0.0f32; 4];
                    for (v, w) in vertices.iter().zip(weights) {
                        for (ch, value) in v.color.to_array().iter().enumerate() {
                            color[ch] += *value as f32 / 255.0 * w;
                        }
                    }
                    let alpha = color[3] * texel[3] as f32 / 255.0;
                    let pixel = &mut pixels[y as usize * width + x as usize];
                    for ch in 0..3 {
                        pixel[ch] = (color[ch] * texel[ch] as f32
                            + pixel[ch] as f32 * (1.0 - alpha))
                            .clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }
    }
    pixels
}

#[test]
fn dropdown_reveals_then_collapses_without_executing_closed_menu_actions() {
    let mut h = Harness::new();
    h.idle(3);
    h.click(h.anchor.center());
    h.idle(2);
    let opening = h.state();
    let amount = opening.motion.value(h.time);
    assert!(amount > 0.0 && amount < 1.0);
    assert!(opening.snapshot.is_some());
    h.idle(16);
    assert_eq!(h.state().motion.value(h.time), 1.0);
    h.click(h.option.center());
    assert_eq!(h.selected, 1);
    assert!(!egui::Popup::is_id_open(&h.ctx, h.popup));
    let calls = h.calls;
    h.idle(2);
    let closing = h.state();
    assert!(
        closing.snapshot.is_some(),
        "Collapse must remain painted after native dismissal"
    );
    assert!(closing.motion.value(h.time) > 0.0 && closing.motion.value(h.time) < 1.0);
    assert_eq!(
        h.calls, calls,
        "Closing snapshot must never execute option callbacks"
    );
    h.idle(12);
    assert!(h.state().snapshot.is_none());
    assert!(
        h.ctx
            .data(|d| d.get_temp::<Vec<Id>>(registry_id()))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn escape_and_outside_click_dismiss_and_removed_controls_finish_their_animation() {
    let mut h = Harness::new();
    h.idle(3);
    h.click(h.anchor.center());
    h.idle(16);
    h.frame(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }]);
    assert!(!egui::Popup::is_id_open(&h.ctx, h.popup));
    h.idle(12);
    h.click(h.anchor.center());
    h.idle(16);
    h.click(egui::pos2(325.0, 240.0));
    assert!(!egui::Popup::is_id_open(&h.ctx, h.popup));
    h.idle(12);
    h.click(h.anchor.center());
    h.idle(16);
    h.show_control = false;
    h.idle(2);
    assert!(!egui::Popup::is_id_open(&h.ctx, h.popup));
    h.idle(12);
    assert!(h.state().snapshot.is_none());
}

#[test]
fn rapidly_reversing_motion_is_continuous_and_upward_popups_reveal_from_the_anchor() {
    let mut motion = Motion::default();
    motion.target(true, 0.0);
    let before = motion.value(0.045);
    motion.target(false, 0.045);
    assert_eq!(before, motion.value(0.045));
    let closing = motion.value(0.07);
    motion.target(true, 0.07);
    assert_eq!(closing, motion.value(0.07));
    assert_eq!(motion.value(1.0), 1.0);
    let rect = Rect::from_min_max(egui::pos2(10.0, 20.0), egui::pos2(200.0, 200.0));
    let below = reveal(
        rect,
        Rect::from_min_max(egui::pos2(10.0, 0.0), egui::pos2(200.0, 15.0)),
        0.5,
    );
    let above = reveal(
        rect,
        Rect::from_min_max(egui::pos2(10.0, 205.0), egui::pos2(200.0, 240.0)),
        0.5,
    );
    assert_eq!(below.top(), rect.top() - 8.0);
    assert_eq!(above.bottom(), rect.bottom() + 8.0);
    assert!(below.bottom() < above.bottom());
}
