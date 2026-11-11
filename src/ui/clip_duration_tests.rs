use super::*;

struct Harness {
    ctx: egui::Context,
    settings: Settings,
    time: f64,
    open: bool,
    positions: HashMap<String, egui::Pos2>,
}
impl Harness {
    fn frame(&mut self, events: Vec<egui::Event>) {
        self.time += 1.0 / 60.0;
        let output = crate::ui::test_render::run(
            &self.ctx,
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(900.0, 650.0),
                )),
                time: Some(self.time),
                events,
                ..Default::default()
            },
            |root| {
                egui::CentralPanel::default().show(root, |ui| {
                    let anchor = ui.button("Clips");
                    super::super::widgets::quick_panel(
                        ui,
                        &anchor,
                        egui::Id::unique("duration-panel"),
                        &mut self.open,
                        false,
                        |ui| {
                            ui.horizontal(|ui| {
                                let _ = ui.button("F8");
                                duration_selector(
                                    ui,
                                    &mut self.settings,
                                    "clip-menu-duration",
                                    126.0,
                                );
                            });
                        },
                    );
                });
                super::super::motion::finish(root.ctx());
            },
        );
        fn collect(shape: &egui::Shape, positions: &mut HashMap<String, egui::Pos2>) {
            match shape {
                egui::Shape::Text(text) => {
                    positions.insert(
                        text.galley.text().into(),
                        text.pos + text.galley.size() * 0.5,
                    );
                }
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        collect(shape, positions);
                    }
                }
                _ => {}
            }
        }
        self.positions.clear();
        for shape in &output.shapes {
            collect(&shape.shape, &mut self.positions);
        }
    }
    fn idle(&mut self) {
        for _ in 0..18 {
            self.frame(vec![]);
        }
    }
    fn click(&mut self, label: &str) {
        let pos = self.positions[label];
        for pressed in [true, false] {
            self.frame(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ]);
        }
    }
}

#[test]
fn duration_dropdown_applies_new_values_and_survives_shortcut_sync_and_reload() {
    let mut settings = Settings::default();
    settings.edit_capture_bindings();
    let mut h = Harness {
        ctx: egui::Context::default(),
        settings,
        time: 0.0,
        open: true,
        positions: HashMap::new(),
    };
    h.idle();
    for (seconds, label) in [(15, "15 s"), (120, "2 min"), (30, "30 s")] {
        h.click(&duration_label(h.settings.replay_seconds));
        h.idle();
        h.click(label);
        h.idle();
        assert!(
            h.open,
            "Selecting a duration must keep its parent panel open"
        );
        h.settings.sync_primary_bindings();
        assert_eq!(h.settings.replay_seconds, seconds);
        let mut restored: Settings =
            serde_json::from_str(&serde_json::to_string(&h.settings).unwrap()).unwrap();
        restored.sync_primary_bindings();
        assert_eq!(restored.replay_seconds, seconds);
        assert_eq!(restored.capture_bindings()[0].clip_seconds, seconds);
    }
}
