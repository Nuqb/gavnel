use super::*;

#[derive(Clone)]
pub(super) enum Destination {
    Page(Page, SettingsPage),
    Player(PathBuf),
}

#[derive(Clone, Copy, PartialEq)]
enum Choice {
    Save,
    Discard,
    Cancel,
}

impl Editor {
    fn dirty(&self) -> bool {
        self.project.is_some() && self.project != self.saved_project
    }

    pub(in crate::ui) fn clear_pending_leave(&mut self) {
        self.leave = None;
    }

    pub(in crate::ui) fn defer_player(&mut self, path: PathBuf) -> bool {
        self.defer_leave(Destination::Player(path))
    }

    pub(super) fn defer_leave(&mut self, destination: Destination) -> bool {
        if self.saving_video.is_some() {
            self.leave = None;
            return false;
        }
        if !self.dirty() {
            return false;
        }
        if self.leave.is_none() {
            self.leave = Some(destination);
            self.play_requested = false;
            if let Some((_, cancel)) = self.preview_job.take() {
                cancel.store(true, Ordering::Release);
            }
        }
        true
    }

    fn resolve_leave(&mut self, choice: Choice) -> Option<Destination> {
        self.leave.as_ref()?;
        match choice {
            Choice::Cancel => {
                self.leave = None;
                return None;
            }
            Choice::Save => {
                self.save_editing = false;
                self.save_open = true;
                return None;
            }
            Choice::Discard => {
                self.project = self.saved_project.clone();
                self.undo.clear();
                self.redo.clear();
                self.pending = None;
                self.last_frame = None;
            }
        }
        self.leave.take()
    }
}

impl Gavnel {
    pub(in crate::ui) fn guard_editor_navigation(&mut self, previous: (Page, SettingsPage)) {
        if previous.0 != Page::Editor || self.page == Page::Editor {
            return;
        }
        self.player = None;
        let destination = if self.page == Page::Player {
            self.editor
                .as_ref()
                .and_then(|e| e.return_player.clone())
                .map(Destination::Player)
                .unwrap_or(Destination::Page(Page::Clips, self.settings_page))
        } else {
            Destination::Page(self.page, self.settings_page)
        };
        if self
            .editor
            .as_mut()
            .is_some_and(|e| e.defer_leave(destination.clone()))
        {
            (self.page, self.settings_page) = previous;
        } else if let Destination::Player(path) = destination {
            self.open_player(&path);
        }
    }

    pub(in crate::ui) fn editor_leave_dialog(&mut self, ctx: &egui::Context) {
        let Some(editor) = self.editor.as_mut().filter(|e| e.leave.is_some()) else {
            return;
        };
        self.player = None;
        if editor.save_open || editor.saving_video.is_some() {
            return;
        }
        let destination = if editor.dirty() {
            let Some(choice) = leave_dialog(ctx, &editor.error) else {
                return;
            };
            editor.resolve_leave(choice)
        } else {
            editor.leave.take()
        };
        if let Some(destination) = destination {
            match destination {
                Destination::Page(page, settings_page) => {
                    self.page = page;
                    self.settings_page = settings_page;
                }
                Destination::Player(path) => self.open_player(&path),
            }
        }
    }
}

fn leave_dialog(ctx: &egui::Context, error: &str) -> Option<Choice> {
    let response = egui::Modal::new(egui::Id::unique("editor-unsaved-changes"))
        .backdrop_color(Color32::from_black_alpha(180))
        .frame(
            egui::Frame::new()
                .fill(PANEL)
                .stroke(egui::Stroke::new(1.0, LINE))
                .corner_radius(16.0)
                .inner_margin(26.0),
        )
        .show(ctx, |ui| {
            ui.set_width(360.0);
            ui.label(RichText::new(tr("Save changes?")).size(22.0).strong());
            ui.add_space(20.0);
            if !error.is_empty() {
                ui.colored_label(DANGER, error);
                ui.add_space(12.0);
            }
            ui.horizontal(|ui| {
                if views::orange_button(ui, tr("Save")).clicked() {
                    return Some(Choice::Save);
                }
                if ui.button(tr("Discard")).clicked() {
                    return Some(Choice::Discard);
                }
                if ui.button(tr("Cancel")).clicked() {
                    return Some(Choice::Cancel);
                }
                None
            })
            .inner
        });
    response
        .inner
        .or_else(|| response.should_close().then_some(Choice::Cancel))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edited() -> Editor {
        let mut editor = Editor::new(PathBuf::new());
        let project = EditProject::new(vec![]);
        editor.saved_project = Some(project.clone());
        editor.project = Some(project);
        // A project setting change is sufficient; navigation must compare saved content.
        editor.project.as_mut().unwrap().name = "Changed".into();
        editor
    }

    #[test]
    fn saving_video_can_leave_the_editor_without_a_discard_prompt() {
        let mut editor = edited();
        editor.saving_video = Some(Arc::new(Mutex::new(JobProgress::default())));
        assert!(!editor.defer_leave(Destination::Page(Page::Clips, SettingsPage::Recording)));
        assert!(editor.leave.is_none());
    }

    #[test]
    fn discard_and_cancel_do_not_save() {
        let mut editor = edited();
        assert!(editor.defer_leave(Destination::Page(Page::Clips, SettingsPage::Recording)));
        assert!(editor.resolve_leave(Choice::Cancel).is_none());
        assert!(editor.dirty());
        assert!(editor.defer_leave(Destination::Page(Page::Clips, SettingsPage::Recording)));
        assert!(editor.resolve_leave(Choice::Discard).is_some());
        assert!(!editor.dirty());
        assert!(!editor.defer_leave(Destination::Page(Page::Settings, SettingsPage::Games)));
    }

    #[test]
    fn player_destination_survives_unsaved_changes_prompt() {
        let mut editor = edited();
        let path = PathBuf::from("clip.mp4");
        assert!(editor.defer_leave(Destination::Player(path.clone())));
        assert!(
            matches!(editor.resolve_leave(Choice::Discard), Some(Destination::Player(p)) if p == path)
        );
    }

    #[test]
    fn save_opens_video_choices_and_keeps_pending_destination() {
        let mut editor = edited();
        editor.defer_leave(Destination::Page(Page::Clips, SettingsPage::Recording));
        assert!(editor.resolve_leave(Choice::Save).is_none());
        assert!(editor.dirty());
        assert!(editor.leave.is_some());
        assert!(editor.save_open);
        assert!(editor.error.is_empty());
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    #[test]
    fn save_choice_and_drop_never_create_project_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut editor = Editor::new(dir.path().to_owned());
        let original = EditProject::new(vec![]);
        editor.saved_project = Some(original.clone());
        editor.project = Some(original);
        editor.project.as_mut().unwrap().name = "Edited".into();
        editor.defer_leave(Destination::Page(Page::Clips, SettingsPage::Recording));
        assert!(editor.resolve_leave(Choice::Save).is_none());
        assert!(editor.save_open);
        assert!(editor.dirty());
        drop(editor);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn undo_to_saved_content_does_not_prompt() {
        let mut editor = Editor::new(PathBuf::new());
        let original = EditProject::new(vec![]);
        editor.project = Some(original.clone());
        editor.saved_project = Some(original.clone());
        editor.project.as_mut().unwrap().name = "Edited".into();
        editor.record_edit(original, false);
        assert!(editor.dirty());
        editor.undo();
        assert!(!editor.defer_leave(Destination::Page(Page::Clips, SettingsPage::Recording)));
    }
}

#[cfg(test)]
mod dialog_tests {
    use super::*;
    fn labels(shape: &egui::Shape, output: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(text) => {
                output.push((text.galley.text().into(), text.visual_bounding_rect()))
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    labels(shape, output);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn popup_buttons_are_clickable() {
        for (label, expected) in [
            ("Save", Choice::Save),
            ("Discard", Choice::Discard),
            ("Cancel", Choice::Cancel),
        ] {
            let ctx = egui::Context::default();
            let mut result = None;
            let mut frame = |events| {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            Vec2::new(800.0, 600.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |_| {
                        result = leave_dialog(&ctx, "");
                    },
                );
                output.textures_delta.clear();
                output
            };
            frame(vec![]);
            let output = frame(vec![]);
            let mut texts = vec![];
            for shape in output.shapes {
                labels(&shape.shape, &mut texts);
            }
            let position = texts
                .iter()
                .find(|(text, _)| text == label)
                .unwrap()
                .1
                .center();
            frame(vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
            frame(vec![egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }]);
            assert!(result == Some(expected), "{label} should select its action");
        }
    }
}
