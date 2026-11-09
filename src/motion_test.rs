//! Optional deterministic moving source for local capture diagnostics.
use eframe::egui;
pub fn run() -> anyhow::Result<()> {
    struct Motion {
        start: std::time::Instant,
    }
    impl eframe::App for Motion {
        fn ui(&mut self, root: &mut egui::Ui, _: &mut eframe::Frame) {
            let ctx = &root.ctx().clone();
            let t = self.start.elapsed().as_secs_f32();
            egui::CentralPanel::default().show(root, |ui| {
                let rect = ui.max_rect();
                for x in 0..16 {
                    let value = (((t * 2.0 + x as f32 / 16.0).fract() * 255.0) as u8).max(20);
                    let r = egui::Rect::from_min_size(
                        egui::pos2(rect.left() + rect.width() * x as f32 / 16.0, rect.top()),
                        egui::vec2(rect.width() / 16.0 + 1.0, rect.height()),
                    );
                    ui.painter().rect_filled(
                        r,
                        0.0,
                        egui::Color32::from_rgb(value, 80, 255 - value),
                    );
                }
                let x = rect.left() + rect.width() * (t * 0.7).fract();
                ui.painter().circle_filled(
                    egui::pos2(x, rect.center().y),
                    70.0,
                    egui::Color32::WHITE,
                );
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("Gavnel motion test · {t:.3}s"),
                    egui::FontId::proportional(36.0),
                    egui::Color32::BLACK,
                );
            });
            if t > 45.0 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint();
        }
    }
    eframe::run_native(
        "Gavnel motion diagnostic",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1100.0, 700.0])
                .with_position([80.0, 80.0]),
            ..Default::default()
        },
        Box::new(|_| {
            Ok(Box::new(Motion {
                start: std::time::Instant::now(),
            }))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Motion diagnostic: {e}"))
}
