use super::worker::Worker;
use super::*;

impl Worker {
    pub(super) fn process_highlights(&mut self) {
        let Some(rec) = &mut self.recording else {
            return;
        };
        if let Some(runtime) = &rec.highlights {
            for mut event in runtime.events.try_iter() {
                if !runtime.enabled.contains(&event.kind) {
                    continue;
                }
                if let Some(session) = &rec.session {
                    if let Ok(seconds) = session.bookmark(event.ticks) {
                        event.ticks = (seconds * TICKS as f64) as u64;
                        if let Err(e) = session.event(&event) {
                            self.input.shared.lock().unwrap().message = e.to_string();
                        }
                    }
                } else {
                    crate::highlights::merge_window(
                        &mut rec.pending_highlights,
                        event,
                        rec.highlight_settings.before,
                        rec.highlight_settings.after,
                    );
                }
            }
        }
        self.flush_highlights(false);
    }
    pub(super) fn flush_highlights(&mut self, finish: bool) {
        let Some(rec) = &mut self.recording else {
            return;
        };
        let now = rec.clock.now();
        let mut index = 0;
        while index < rec.pending_highlights.len() {
            if !finish && rec.pending_highlights[index].end > now {
                index += 1;
                continue;
            }
            if self.saves.pending() >= 2 {
                if finish {
                    rec.pending_highlights.clear();
                    self.input.shared.lock().unwrap().message =
                        "Automatic highlight could not be queued; save queue is full.".into();
                }
                break;
            }
            let window = rec.pending_highlights.remove(index);
            let seconds = window
                .end
                .min(now)
                .saturating_sub(window.start)
                .div_ceil(TICKS) as u32;
            match rec
                .buffer
                .snapshot_until(&rec.label, seconds, window.end.min(now))
            {
                Ok(snapshot) => {
                    if let Err(e) = crate::storage::check_limit(
                        &self.settings.clips_dir,
                        self.settings.storage_limit_gb,
                        snapshot
                            .packets
                            .iter()
                            .map(|p| p.data.bytes.len() as u64)
                            .sum(),
                    ) {
                        self.input.shared.lock().unwrap().message = e.to_string();
                        continue;
                    }
                    let events = window
                        .events
                        .into_iter()
                        .filter(|e| e.ticks >= snapshot.start && e.ticks <= snapshot.end)
                        .map(|mut e| {
                            e.ticks = e.ticks.saturating_sub(snapshot.start);
                            e
                        })
                        .collect();
                    self.clip_id = self.clip_id.wrapping_add(1).max(1);
                    if let Err(e) = self.saves.send(SaveJob::Clip {
                        id: self.clip_id,
                        snapshot,
                        folder: self.settings.clips_dir.clone(),
                        events,
                    }) {
                        self.input.shared.lock().unwrap().message = e.to_string();
                    }
                }
                Err(e) => {
                    self.input.shared.lock().unwrap().message =
                        format!("Automatic highlight unavailable: {e}")
                }
            }
        }
    }
}
