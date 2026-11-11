//! Bounded background save queue. Owns pending counts and drains on normal exit.
use super::*;

pub(super) struct SaveQueue {
    sender: Sender<SaveJob>,
    pending: Arc<AtomicUsize>,
    worker: JoinHandle<()>,
}

impl SaveQueue {
    pub fn new(status: Arc<Mutex<Status>>, notify: Notify, feedback: Option<Feedback>) -> Self {
        let (sender, receiver) = bounded::<SaveJob>(1);
        let pending = Arc::new(AtomicUsize::new(0));
        let count = pending.clone();
        let worker = std::thread::Builder::new()
            .name("gavnel-clip-writer".into())
            .spawn(move || {
                for job in receiver {
                    let (result, event) = save(job);
                    if let Some(feedback) = &feedback {
                        feedback.send(event);
                    }
                    count.fetch_sub(1, Ordering::AcqRel);
                    let mut state = status.lock().unwrap();
                    match result {
                        Ok(path) => {
                            state.message = format!(
                                "Saved {}",
                                path.file_name().unwrap_or_default().to_string_lossy()
                            );
                            state.last_clip = Some(path);
                            state.saved_count += 1;
                        }
                        Err(error) => state.fail(format!("Capture could not be saved: {error:#}")),
                    }
                    state.saving = count.load(Ordering::Relaxed);
                    drop(state);
                    notify.wake();
                }
            })
            .expect("Could not start clip writer");
        Self {
            sender,
            pending,
            worker,
        }
    }

    pub fn pending(&self) -> usize {
        self.pending.load(Ordering::Acquire)
    }

    pub fn send(&self, job: SaveJob) -> Result<()> {
        self.pending.fetch_add(1, Ordering::AcqRel);
        if self.sender.try_send(job).is_err() {
            self.pending.fetch_sub(1, Ordering::AcqRel);
            anyhow::bail!("The save queue is full. Try again shortly.");
        }
        Ok(())
    }

    pub fn finish(self) {
        drop(self.sender);
        let _ = self.worker.join();
    }
}

fn save(job: SaveJob) -> (Result<PathBuf>, FeedbackEvent) {
    match job {
        SaveJob::Clip {
            id,
            snapshot,
            folder,
            events,
        } => {
            let result = write_clip(&snapshot, &folder).and_then(|path| {
                crate::highlights::save_events(&path, &events)?;
                for event in &events {
                    crate::bookmarks::append(&path, event.ticks as f64 / TICKS as f64)?;
                }
                Ok(path)
            });
            let event = FeedbackEvent::Finished {
                id,
                success: result.is_ok(),
            };
            (result, event)
        }
        SaveJob::Screenshot {
            image,
            folder,
            source,
            limit,
        } => {
            let result = crate::screenshot::save(&image, &folder, &source, limit);
            let event = FeedbackEvent::ScreenshotFinished {
                source,
                error: result.as_ref().err().map(|error| format!("{error:#}")),
            };
            (result, event)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screenshot_feedback_reports_the_actual_save_result() {
        let folder = tempfile::tempdir().unwrap();
        for valid in [true, false] {
            let (result, event) = save(SaveJob::Screenshot {
                image: crate::screenshot::Screenshot {
                    width: 1,
                    height: 1,
                    rgba: if valid { vec![255; 4] } else { Vec::new() },
                },
                folder: folder.path().to_owned(),
                source: "Desktop".into(),
                limit: None,
            });
            assert_eq!(result.is_ok(), valid);
            let FeedbackEvent::ScreenshotFinished { source, error } = event else {
                panic!("Screenshot saves must send screenshot feedback");
            };
            assert_eq!(source, "Desktop");
            assert_eq!(error.is_none(), valid);
            if let Some(error) = error {
                assert!(error.contains("Invalid screenshot pixels"));
            }
        }
    }

    #[test]
    fn normal_shutdown_drains_saves_and_reports_success_or_failure() {
        for valid in [true, false] {
            let folder = tempfile::tempdir().unwrap();
            let status = Arc::new(Mutex::new(Status::default()));
            let queue = SaveQueue::new(
                status.clone(),
                Notify {
                    ctx: eframe::egui::Context::default(),
                    visible: Arc::new(AtomicBool::new(false)),
                },
                None,
            );
            queue
                .send(SaveJob::Screenshot {
                    image: crate::screenshot::Screenshot {
                        width: 1,
                        height: 1,
                        rgba: if valid {
                            vec![255, 0, 0, 255]
                        } else {
                            Vec::new()
                        },
                    },
                    folder: folder.path().to_owned(),
                    source: "Queue test".into(),
                    limit: None,
                })
                .unwrap();
            queue.finish();
            let state = status.lock().unwrap();
            assert_eq!(state.saving, 0);
            assert_eq!(state.saved_count, u64::from(valid));
            if valid {
                assert!(state.last_clip.as_ref().unwrap().is_file());
            } else {
                assert!(state.message.contains("could not be saved"));
                assert_eq!(state.last_error.as_ref(), Some(&state.message));
                assert!(state.last_clip.is_none());
            }
        }
    }
}
