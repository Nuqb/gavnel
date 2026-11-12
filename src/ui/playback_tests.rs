use super::*;

struct Harness {
    ctx: egui::Context,
    size: Vec2,
    time: f64,
    state: playback_controls::State<'static>,
    textures: HashMap<egui::TextureId, egui::ColorImage>,
}
impl Harness {
    fn new(size: Vec2) -> Self {
        let ctx = egui::Context::default();
        super::super::theme::apply(&ctx);
        Self {
            ctx,
            size,
            time: 0.0,
            state: playback_controls::State {
                duration: 120.0,
                position: 30.0,
                volume: 1.0,
                muted: false,
                paused: true,
                ended: false,
                fullscreen: false,
                bookmarks: &[],
            },
            textures: HashMap::new(),
        }
    }
    fn surface(&self) -> egui::Rect {
        if self.state.fullscreen {
            egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)
        } else {
            egui::Rect::from_min_max(
                egui::pos2(28.0, 90.0),
                egui::pos2(self.size.x - 28.0, self.size.y - 28.0),
            )
        }
    }
    fn frame(&mut self, events: Vec<egui::Event>) -> (egui::FullOutput, playback_controls::Action) {
        self.time += 1.0 / 60.0;
        let mut action = playback_controls::Action::default();
        let surface = self.surface();
        let (video, controls) = surface_rects(surface, self.state.bookmarks);
        let mut output = self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
                events,
                time: Some(self.time),
                ..Default::default()
            },
            |root| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(BG))
                    .show(root, |ui| {
                        if !self.state.fullscreen {
                            ui.label(RichText::new("Recording.mp4").size(20.0));
                        }
                        ui.painter().rect_filled(surface, 8.0, Color32::BLACK);
                        ui.painter()
                            .rect_filled(video, 8.0, Color32::from_rgb(30, 75, 100));
                        for stripe in 0..8 {
                            let x = video.left() + video.width() * stripe as f32 / 8.0;
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(
                                    egui::pos2(x, video.top()),
                                    Vec2::new(video.width() / 16.0, video.height()),
                                ),
                                0.0,
                                Color32::from_rgb(36, 90, 110),
                            );
                        }
                        ui.painter().text(
                            video.center(),
                            egui::Align2::CENTER_CENTER,
                            "Video",
                            egui::FontId::proportional(18.0),
                            MUTED,
                        );
                        ui.scope_builder(
                            egui::UiBuilder::new()
                                .max_rect(controls)
                                .layout(egui::Layout::top_down(egui::Align::Min)),
                            |ui| {
                                action = playback_controls::show(ui, &mut self.state);
                            },
                        );
                    });
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
        (output, action)
    }
    fn click(&mut self, point: egui::Pos2) -> playback_controls::Action {
        self.frame(vec![
            egui::Event::PointerMoved(point),
            egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        self.frame(vec![egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }])
        .1
    }
}

fn icon(
    output: &egui::FullOutput,
    ctx: &egui::Context,
    name: &'static str,
    svg: &[u8],
) -> egui::Rect {
    fn find(shape: &egui::Shape, id: egui::TextureId) -> Option<egui::Rect> {
        match shape {
            egui::Shape::Mesh(mesh) if mesh.texture_id == id => Some(mesh.calc_bounds()),
            egui::Shape::Rect(rect) if rect.fill_texture_id() == id => Some(rect.rect),
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, id)),
            _ => None,
        }
    }
    let size = if name.starts_with("nav-") { 36 } else { 32 };
    let id = crate::icons::texture(ctx, name, svg, size).id();
    output
        .shapes
        .iter()
        .find_map(|shape| find(&shape.shape, id))
        .unwrap_or_else(|| panic!("Missing {name}"))
}

#[test]
fn bottom_controls_keep_volume_next_to_play_and_fullscreen_at_the_right() {
    for size in [Vec2::new(800.0, 600.0), Vec2::new(1280.0, 820.0)] {
        let mut h = Harness::new(size);
        h.frame(vec![]);
        let (output, _) = h.frame(vec![]);
        let play = icon(
            &output,
            &h.ctx,
            "nav-play",
            include_bytes!("../../assets/icons/play.svg"),
        );
        let volume = icon(
            &output,
            &h.ctx,
            "nav-volume",
            include_bytes!("../../assets/icons/volume.svg"),
        );
        let full = icon(
            &output,
            &h.ctx,
            "fullscreen",
            include_bytes!("../../assets/icons/fullscreen.svg"),
        );
        let (_, bar) = surface_rects(h.surface(), h.state.bookmarks);
        assert!(bar.contains_rect(play) && bar.contains_rect(volume) && bar.contains_rect(full));
        assert!(volume.left() > play.right() && volume.center().x - play.center().x < 50.0);
        assert!((play.center().y - volume.center().y).abs() < 1.0);
        assert!(full.right() > bar.right() - 40.0);
        assert!(h.click(play.center()).toggle_play);
        assert!(h.click(volume.center()).audio_changed);
        assert!(h.state.muted);
        assert!(h.click(full.center()).toggle_fullscreen);
    }
}

#[test]
fn seek_and_volume_sliders_and_bookmarks_remain_interactive() {
    let mut h = Harness::new(Vec2::new(1000.0, 650.0));
    h.frame(vec![]);
    let (output, _) = h.frame(vec![]);
    let (_, bar) = surface_rects(h.surface(), h.state.bookmarks);
    assert!(
        h.click(egui::pos2(
            bar.left() + bar.width() * 0.75,
            bar.top() + 20.0,
        ))
        .seek
        .is_some()
    );
    assert!((80.0..100.0).contains(&h.state.position));
    let volume = icon(
        &output,
        &h.ctx,
        "nav-volume",
        include_bytes!("../../assets/icons/volume.svg"),
    );
    for _ in 0..20 {
        h.frame(vec![egui::Event::PointerMoved(volume.center())]);
    }
    h.click(egui::pos2(volume.right() + 34.0, volume.center().y));
    assert!(h.state.volume < 0.8);
    h.state.bookmarks = &[15.0, 65.0];
    h.frame(vec![]);
    let (output, _) = h.frame(vec![]);
    fn label(shape: &egui::Shape) -> Option<egui::Rect> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == "2 \u{00b7} 1:05" => {
                Some(text.visual_bounding_rect())
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(label),
            _ => None,
        }
    }
    let mark = output
        .shapes
        .iter()
        .find_map(|shape| label(&shape.shape))
        .unwrap();
    assert_eq!(h.click(mark.center()).seek, Some(65.0));
}

#[test]
fn volume_hover_moves_time_and_keeps_fullscreen_anchored() {
    fn time_rect(output: &egui::FullOutput) -> egui::Rect {
        output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "0:30 / 2:00" => {
                    Some(text.visual_bounding_rect())
                }
                _ => None,
            })
            .unwrap()
    }
    let mut h = Harness::new(Vec2::new(1000.0, 650.0));
    h.frame(vec![]);
    let (before, _) = h.frame(vec![]);
    let volume = icon(
        &before,
        &h.ctx,
        "nav-volume",
        include_bytes!("../../assets/icons/volume.svg"),
    );
    let full = icon(
        &before,
        &h.ctx,
        "fullscreen",
        include_bytes!("../../assets/icons/fullscreen.svg"),
    );
    let time = time_rect(&before);
    for _ in 0..4 {
        h.frame(vec![egui::Event::PointerMoved(volume.center())]);
    }
    let (opening, _) = h.frame(vec![]);
    assert!(time_rect(&opening).left() > time.left() + 40.0);
    for _ in 0..20 {
        h.frame(vec![egui::Event::PointerMoved(volume.center())]);
    }
    let (expanded, _) = h.frame(vec![]);
    assert!(time_rect(&expanded).left() > time.left() + 70.0);
    assert_eq!(
        full,
        icon(
            &expanded,
            &h.ctx,
            "fullscreen",
            include_bytes!("../../assets/icons/fullscreen.svg")
        )
    );
    for _ in 0..20 {
        h.frame(vec![egui::Event::PointerMoved(h.surface().center())]);
    }
    let (collapsed, _) = h.frame(vec![]);
    assert!((time_rect(&collapsed).left() - time.left()).abs() < 1.0);
}

#[test]
fn volume_thumb_outline_fits_at_both_endpoints() {
    let mut h = Harness::new(Vec2::new(1000.0, 650.0));
    h.frame(vec![]);
    let (before, _) = h.frame(vec![]);
    let volume = icon(
        &before,
        &h.ctx,
        "nav-volume",
        include_bytes!("../../assets/icons/volume.svg"),
    );
    for _ in 0..20 {
        h.frame(vec![egui::Event::PointerMoved(volume.center())]);
    }
    for value in [0.0, 1.0] {
        h.state.volume = value;
        let (output, _) = h.frame(vec![]);
        let thumb = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Circle(circle)
                    if (circle.center.y - volume.center().y).abs() < 2.0 =>
                {
                    Some(circle.center)
                }
                _ => None,
            })
            .expect("volume thumb");
        // Hover expands the painted handle beyond its normal endpoint bounds.
        h.frame(vec![egui::Event::PointerMoved(thumb)]);
        let (output, _) = h.frame(vec![]);
        let shape = output
            .shapes
            .iter()
            .find(|shape| {
                matches!(&shape.shape,
                    egui::Shape::Circle(circle) if (circle.center.y - volume.center().y).abs() < 2.0
                )
            })
            .unwrap();
        assert!(
            shape
                .clip_rect
                .contains_rect(shape.shape.visual_bounding_rect())
        );
    }
}

#[test]
fn fullscreen_commands_enter_and_restore_the_window() {
    let ctx = egui::Context::default();
    let mut enabled = false;
    fullscreen(&ctx, &mut enabled, true);
    assert!(enabled);
    let output = ctx.run_pass(Default::default(), |_| {});
    assert!(
        output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::Fullscreen(true))
    );
    output.drop_without_applying_deltas();
    fullscreen(&ctx, &mut enabled, false);
    assert!(!enabled);
    let output = ctx.run_pass(Default::default(), |_| {});
    assert!(
        output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::Fullscreen(false))
    );
    output.drop_without_applying_deltas();
}

#[test]
#[ignore = "renders the player control layout for visual review"]
fn export_playback_previews() {
    std::fs::create_dir_all("work/player-previews").unwrap();
    for (fullscreen, expanded) in [(false, false), (true, false), (false, true)] {
        let mut h = Harness::new(Vec2::new(1000.0, 650.0));
        h.state.fullscreen = fullscreen;
        for _ in 0..3 {
            h.frame(vec![]);
        }
        if expanded {
            let (output, _) = h.frame(vec![]);
            let volume = icon(
                &output,
                &h.ctx,
                "nav-volume",
                include_bytes!("../../assets/icons/volume.svg"),
            );
            for _ in 0..20 {
                h.frame(vec![egui::Event::PointerMoved(volume.center())]);
            }
        }
        let (output, _) = h.frame(vec![]);
        let pixels = super::super::test_render::raster(&h.ctx, h.size, &h.textures, output);
        let file = std::fs::File::create(format!(
            "work/player-previews/fullscreen-{fullscreen}-volume-{expanded}.png"
        ))
        .unwrap();
        let mut encoder = png::Encoder::new(file, 1000, 650);
        encoder.set_color(png::ColorType::Rgb);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels.into_iter().flatten().collect::<Vec<_>>())
            .unwrap();
    }
}

#[test]
fn escape_leaves_fullscreen_before_returning_to_the_library() {
    let ctx = egui::Context::default();
    let mut current = true;
    assert!(!fullscreen_keys(&ctx, &mut current, true, false));
    assert!(!current);
    assert!(fullscreen_keys(&ctx, &mut current, true, false));
    assert!(!fullscreen_keys(&ctx, &mut current, false, true));
    assert!(current);
}

#[test]
fn escape_still_exits_fullscreen_after_focusing_a_slider() {
    let ctx = egui::Context::default();
    let mut enabled = true;
    let mut volume = 0.5_f64;
    let mut output = ctx.run_ui(Default::default(), |root| {
        egui::CentralPanel::default().show(root, |ui| {
            ui.add(egui::Slider::new(&mut volume, 0.0..=1.0))
                .request_focus();
        });
    });
    output.textures_delta.clear();
    assert!(ctx.egui_wants_keyboard_input());
    let mut exited = false;
    let mut output = ctx.run_ui(
        egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        },
        |root| {
            egui::CentralPanel::default().show(root, |ui| {
                exited = exit_fullscreen_on_escape(ui, &mut enabled);
            });
        },
    );
    output.textures_delta.clear();
    assert!(exited && !enabled);
}

#[test]
fn controls_overlay_the_full_video_instead_of_reserving_a_bar_below_it() {
    let surface = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1280.0, 720.0));
    for bookmarks in [&[][..], &[15.0, 30.0][..]] {
        let (video, controls) = surface_rects(surface, bookmarks);
        assert_eq!(video, surface);
        assert!(video.contains_rect(controls));
        assert_eq!(video.bottom(), controls.bottom());
        assert!(controls.top() > video.center().y);
    }
}

#[test]
fn controls_follow_the_image_edges_inside_letterboxed_and_pillarboxed_surfaces() {
    let surface = egui::Rect::from_min_size(egui::pos2(20.0, 80.0), Vec2::new(1280.0, 960.0));
    for image in [
        surface.shrink2(Vec2::new(0.0, 120.0)),
        surface.shrink2(Vec2::new(370.0, 0.0)),
    ] {
        for bookmarks in [&[][..], &[15.0, 30.0][..]] {
            let (_, controls) = surface_rects(image, bookmarks);
            assert!(image.contains_rect(controls));
            assert_eq!(controls.bottom(), image.bottom());
            assert_eq!(controls.left(), image.left());
            assert_eq!(controls.right(), image.right());
        }
    }
}
