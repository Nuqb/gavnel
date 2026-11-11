use super::*;

fn job() -> Job {
    Job {
        sources: vec![PathBuf::from("source.mp4")],
        target: PathBuf::from("edited.mp4"),
        replacing: false,
        state: Arc::new(Mutex::new(JobProgress {
            fraction: 0.42,
            message: "Saving video".into(),
            result: None,
        })),
        cancel: Arc::new(AtomicBool::new(false)),
        id: 0,
    }
}

#[test]
fn background_job_survives_modal_dismissal_and_publishes_one_notice() {
    let job = job();
    let state = job.state.clone();
    let cancel = job.cancel.clone();
    let mut saves = Saves::default();
    saves.push(job);
    saves.modal = None;
    assert!(saves.busy());
    assert!(saves.status(std::path::Path::new("source.mp4")).is_some());
    assert!(saves.status(std::path::Path::new("edited.mp4")).is_some());
    assert!(
        saves
            .status(std::path::Path::new("unrelated.mp4"))
            .is_none()
    );
    assert!(!cancel.load(Ordering::Acquire));
    state.lock().unwrap().result = Some(Ok(PathBuf::from("edited.mp4")));
    assert_eq!(saves.poll(), vec![PathBuf::from("edited.mp4")]);
    assert!(!saves.busy());
    assert_eq!(saves.notices.len(), 1);
    assert!(saves.poll().is_empty());
    assert_eq!(saves.notices.len(), 1);
}

#[test]
fn failed_save_is_reported_without_a_success_notification_or_refresh() {
    let job = job();
    job.state.lock().unwrap().result = Some(Err("Disk full".into()));
    let mut saves = Saves::default();
    saves.push(job);
    assert!(saves.poll().is_empty());
    assert!(!saves.busy());
    assert!(matches!(&saves.notices[0].result, Err(error) if error == "Disk full"));
}

struct Harness {
    ctx: egui::Context,
    saves: Saves,
    textures: HashMap<egui::TextureId, egui::ColorImage>,
}
impl Harness {
    fn new() -> Self {
        let ctx = egui::Context::default();
        super::super::theme::apply(&ctx);
        let mut saves = Saves::default();
        saves.push(job());
        Self {
            ctx,
            saves,
            textures: HashMap::new(),
        }
    }
    fn frame(
        &mut self,
        events: Vec<egui::Event>,
    ) -> (egui::FullOutput, ProgressAction, Option<PathBuf>) {
        let mut action = ProgressAction::None;
        let mut view = None;
        let mut output = self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1000.0, 650.0),
                )),
                events,
                ..Default::default()
            },
            |root| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(BG).inner_margin(28.0))
                    .show(root, |ui| {
                        ui.heading("Library");
                    });
                action = progress_modal(&self.ctx, &mut self.saves);
                view = completion_banner(&self.ctx, &mut self.saves.notices);
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
        (output, action, view)
    }
    fn click(&mut self, point: egui::Pos2) -> (egui::FullOutput, ProgressAction, Option<PathBuf>) {
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
    }
}

fn label(shape: &egui::Shape, name: &str) -> Option<egui::Rect> {
    match shape {
        egui::Shape::Text(text) if text.galley.text() == name => Some(text.visual_bounding_rect()),
        egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| label(shape, name)),
        _ => None,
    }
}
fn position(output: &egui::FullOutput, name: &str) -> egui::Pos2 {
    output
        .shapes
        .iter()
        .find_map(|shape| label(&shape.shape, name))
        .unwrap_or_else(|| panic!("Missing {name}"))
        .center()
}

#[test]
fn progress_can_continue_in_background_and_completion_view_opens_the_result() {
    let mut h = Harness::new();
    for _ in 0..24 {
        h.frame(vec![]);
    }
    let (output, _, _) = h.frame(vec![]);
    h.click(position(&output, "Continue in background"));
    assert!(h.saves.modal.is_none());
    assert!(h.saves.busy());
    assert!(!h.saves.jobs[0].cancel.load(Ordering::Acquire));
    h.saves.jobs[0].state.lock().unwrap().result = Some(Ok(PathBuf::from("edited.mp4")));
    h.saves.poll();
    for _ in 0..24 {
        h.frame(vec![]);
    }
    let (output, _, _) = h.frame(vec![]);
    let (_, _, view) = h.click(position(&output, "View"));
    assert_eq!(view, Some(PathBuf::from("edited.mp4")));
    assert!(h.saves.notices.is_empty());
}

#[test]
fn back_to_library_keeps_the_export_running() {
    let mut h = Harness::new();
    for _ in 0..24 {
        h.frame(vec![]);
    }
    let (output, _, _) = h.frame(vec![]);
    let (_, action, _) = h.click(position(&output, "Back to library"));
    assert!(action == ProgressAction::Library);
    assert!(h.saves.busy());
    assert!(!h.saves.jobs[0].cancel.load(Ordering::Acquire));
}

#[test]
#[ignore = "renders progress and completion UI for visual review"]
fn export_save_progress_previews() {
    std::fs::create_dir_all("work/editor-previews").unwrap();
    for completed in [false, true] {
        let mut h = Harness::new();
        if completed {
            h.saves.jobs[0].state.lock().unwrap().result = Some(Ok(PathBuf::from("edited.mp4")));
            h.saves.poll();
        }
        for _ in 0..24 {
            h.frame(vec![]);
        }
        let (output, _, _) = h.frame(vec![]);
        let pixels = super::super::test_render::raster(
            &h.ctx,
            Vec2::new(1000.0, 650.0),
            &h.textures,
            output,
        );
        let file =
            std::fs::File::create(format!("work/editor-previews/saving-{completed}.png")).unwrap();
        let mut encoder = png::Encoder::new(file, 1000, 650);
        encoder.set_color(png::ColorType::Rgb);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels.into_iter().flatten().collect::<Vec<_>>())
            .unwrap();
    }
}
