//! The same microphone test controls and meter in Settings and setup.
use super::*;
use egui::{Align, Layout, Pos2, Rect, Sense};

enum Action {
    Start,
    End,
    Play,
    Stop,
}

pub(super) fn show(ui: &mut egui::Ui, settings: &Settings, test: &mut Option<audio::MicTest>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(tr("Mic Test")).strong());
        ui.with_layout(
            if i18n::is_rtl() {
                Layout::left_to_right(Align::Center)
            } else {
                Layout::right_to_left(Align::Center)
            },
            |ui| match controls(ui, test.as_ref()) {
                Some(Action::Start) => *test = Some(audio::MicTest::start(settings.clone())),
                Some(Action::End) => test.as_ref().unwrap().end(),
                Some(Action::Play) => test.as_ref().unwrap().playback(),
                Some(Action::Stop) => test.as_ref().unwrap().stop_playback(),
                None => {}
            },
        );
    });
    if let Some(test) = test {
        if let Some(error) = test.error() {
            ui.colored_label(Color32::LIGHT_RED, error);
        }
        if test.state() != audio::MicTestState::Ready {
            ui.ctx().request_repaint_after(Duration::from_millis(30));
        }
    }
    meter(ui, settings, test.as_ref().map_or(0.0, |test| test.level()));
}

fn controls(ui: &mut egui::Ui, test: Option<&audio::MicTest>) -> Option<Action> {
    match test.map(|test| test.state()) {
        Some(audio::MicTestState::Recording) => {
            if ui.button(tr("Playback")).clicked() {
                return Some(Action::Play);
            }
            if ui.button(tr("End Test")).clicked() {
                return Some(Action::End);
            }
        }
        Some(audio::MicTestState::Playing) => {
            if ui.button(tr("Stop Playback")).clicked() {
                return Some(Action::Stop);
            }
        }
        Some(audio::MicTestState::Ready) | None => {
            if ui.button(tr("Start Test")).clicked() {
                return Some(Action::Start);
            }
            if test.is_some_and(|test| test.has_recording()) && ui.button(tr("Playback")).clicked()
            {
                return Some(Action::Play);
            }
        }
    }
    None
}

fn meter(ui: &mut egui::Ui, settings: &Settings, level: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::hover());
    for i in 0..70 {
        let x = rect.left() + i as f32 * rect.width() / 70.0;
        ui.painter().rect_filled(
            Rect::from_min_size(
                Pos2::new(x, rect.top()),
                Vec2::new((rect.width() / 70.0 - 3.0).max(1.0), rect.height()),
            ),
            0.0,
            if (i as f32) < level * 70.0 {
                ACCENT
            } else {
                MUTED.gamma_multiply(0.45)
            },
        );
    }
    if settings.input_sensitivity_enabled && !settings.push_to_talk {
        let x = rect.left() + rect.width() * ((settings.input_sensitivity_db + 100.0) / 100.0);
        ui.painter().line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            egui::Stroke::new(2.0, TEXT),
        );
    }
    ui.label(
        RichText::new(tr(
            "Mic level: -100 to 0 dB · White line: sensitivity threshold",
        ))
        .size(11.0)
        .color(MUTED),
    );
}
