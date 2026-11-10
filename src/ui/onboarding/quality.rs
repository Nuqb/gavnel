//! Preset choices and advisory PC hardware examples, not measured minimums.
use super::*;

struct Preset {
    title: &'static str,
    height: u32,
    fps: u32,
    bitrate: u32,
    hardware: &'static str,
}

// Recommendations include game headroom; NVENC alone does not require these
// exact CPU/GPU pairings. See docs/development.md for the source and limits.
const PRESETS: [Preset; 4] = [
    Preset {
        title: "Small files",
        height: 720,
        fps: 30,
        bitrate: 8,
        hardware: "NVIDIA GTX 1060\nIntel i5-8400 / AMD Ryzen 5 2600\n8 GB RAM / SSD",
    },
    Preset {
        title: "Smooth HD",
        height: 720,
        fps: 60,
        bitrate: 15,
        hardware: "NVIDIA GTX 1660\nIntel i5-8400 / AMD Ryzen 5 2600\n16 GB RAM / SSD",
    },
    Preset {
        title: "Balanced",
        height: 1080,
        fps: 60,
        bitrate: 20,
        hardware: "NVIDIA RTX 2060\nIntel i5-10400 / AMD Ryzen 5 3600\n16 GB RAM / SSD",
    },
    Preset {
        title: "Extra detail",
        height: 1440,
        fps: 60,
        bitrate: 30,
        hardware: "NVIDIA RTX 3060\nIntel i5-12400 / AMD Ryzen 5 5600\n16 GB RAM / SSD",
    },
];

impl Wizard {
    pub(super) fn quality_page(&mut self, ui: &mut egui::Ui) {
        for preset in &PRESETS {
            let selected = (self.draft.height, self.draft.fps, self.draft.bitrate_mbps)
                == (preset.height, preset.fps, preset.bitrate);
            if quality_option(ui, preset, selected).clicked() {
                self.draft.height = preset.height;
                self.draft.fps = preset.fps;
                self.draft.bitrate_mbps = preset.bitrate;
            }
            ui.add_space(6.0);
        }
        ui.collapsing(tr("Customize quality"), |ui| {
            super::super::widgets::dropdown(
                ui,
                "setup-resolution",
                &mut self.draft.height,
                &[(720, "720p"), (1080, "1080p"), (1440, "1440p")],
            );
            super::super::widgets::dropdown(
                ui,
                "setup-fps",
                &mut self.draft.fps,
                &[(30, "30 FPS"), (60, "60 FPS"), (120, "120 FPS")],
            );
            super::super::widgets::slider(ui, &mut self.draft.bitrate_mbps, 5..=50, "Mbps");
        });
    }
}

fn quality_option(ui: &mut egui::Ui, preset: &Preset, selected: bool) -> egui::Response {
    let width = ui.available_width();
    let label = format!(
        "{}\n{}p \u{b7} {} FPS \u{b7} {} Mbps",
        tr(preset.title),
        preset.height,
        preset.fps,
        preset.bitrate
    );
    ui.horizontal(|ui| {
        let radio = quality_radio(ui, selected);
        radio.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::Role::RadioButton,
                ui.is_enabled(),
                selected,
                tr(preset.title),
            )
        });
        let response = ui.add_sized(
            [width * 0.40, 58.0],
            egui::Button::new(RichText::new(label).size(12.0))
                .fill(if selected { ACCENT_DARK } else { CARD })
                .stroke(Stroke::new(1.0, if selected { ACCENT } else { LINE }))
                .corner_radius(9.0),
        );
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::Role::RadioButton,
                ui.is_enabled(),
                selected,
                tr(preset.title),
            )
        });
        ui.label(RichText::new(preset.hardware).size(12.0).color(MUTED));
        response.union(radio)
    })
    .inner
}

fn quality_radio(ui: &mut egui::Ui, selected: bool) -> egui::Response {
    ui.allocate_ui_with_layout(
        Vec2::new(28.0, 58.0),
        super::layout::leading(Align::Center),
        |ui| super::super::radio::radio(ui, selected, ""),
    )
    .inner
}
