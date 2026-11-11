use super::*;
use crossbeam_channel::{Receiver, Sender, bounded};
use std::sync::{Arc, Mutex, atomic::AtomicBool};

pub enum MediaReply {
    Open(u64, Result<EditProject, String>),
    Frame(u64, Result<eframe::egui::ColorImage, String>),
    Filmstrip(u64, Result<Vec<eframe::egui::ColorImage>, String>),
}
enum Request {
    Open(u64, Vec<PathBuf>),
    Frame(u64, PathBuf, f64),
    Filmstrip(u64, PathBuf),
    Export(
        EditProject,
        ExportOptions,
        Arc<Mutex<JobProgress>>,
        Arc<AtomicBool>,
    ),
}
pub struct MediaService {
    sender: Option<Sender<Request>>,
    export_sender: Option<Sender<Request>>,
    pub replies: Receiver<MediaReply>,
    workers: Vec<std::thread::JoinHandle<()>>,
    cancellations: Mutex<Vec<std::sync::Weak<AtomicBool>>>,
}
impl Default for MediaService {
    fn default() -> Self {
        Self::new()
    }
}
impl MediaService {
    pub fn new() -> Self {
        let (sender, receiver) = bounded::<Request>(2);
        let (tx, replies) = bounded(8);
        let (export_sender, export_receiver) = bounded(1);
        let exporter = std::thread::Builder::new()
            .name("gavnel-export".into())
            .spawn(move || {
                for request in export_receiver {
                    if let Request::Export(project, options, state, cancel) = request {
                        let result = super::render::export(&project, &options, &state, &cancel);
                        let mut p = state.lock().unwrap();
                        p.fraction = if result.is_ok() { 1.0 } else { p.fraction };
                        p.result = Some(result.map_err(|e| e.to_string()));
                    }
                }
            })
            .expect("Could not start export worker");
        let inspector = std::thread::Builder::new()
            .name("gavnel-editor".into())
            .spawn(move || {
                for request in receiver {
                    match request {
                        Request::Open(id, paths) => {
                            let result = (|| -> Result<_> {
                                let mut segments = Vec::new();
                                for path in paths {
                                    segments.push(ClipSegment::new(super::probe(&path)?));
                                }
                                let p = EditProject::new(segments);
                                p.validate()?;
                                Ok(p)
                            })();
                            let _ = tx
                                .try_send(MediaReply::Open(id, result.map_err(|e| e.to_string())));
                        }
                        Request::Filmstrip(id, path) => {
                            let result = super::filmstrip(&path);
                            let _ = tx.try_send(MediaReply::Filmstrip(
                                id,
                                result.map_err(|e| e.to_string()),
                            ));
                        }
                        Request::Frame(id, path, time) => {
                            let result = super::frame(&path, time);
                            let _ = tx
                                .try_send(MediaReply::Frame(id, result.map_err(|e| e.to_string())));
                        }
                        Request::Export(project, options, state, cancel) => {
                            let result = super::render::export(&project, &options, &state, &cancel);
                            let mut p = state.lock().unwrap();
                            p.fraction = if result.is_ok() { 1.0 } else { p.fraction };
                            p.result = Some(result.map_err(|e| e.to_string()));
                        }
                    }
                }
            })
            .expect("Could not start editing worker");
        Self {
            sender: Some(sender),
            export_sender: Some(export_sender),
            replies,
            workers: vec![exporter, inspector],
            cancellations: Mutex::new(Vec::new()),
        }
    }
    pub fn open(&self, id: u64, paths: Vec<PathBuf>) -> Result<()> {
        self.sender
            .as_ref()
            .context("Editor is closed")?
            .try_send(Request::Open(id, paths))
            .map_err(|_| anyhow::anyhow!("The editor is busy. Try again shortly."))
    }
    pub fn frame(&self, id: u64, path: PathBuf, time: f64) -> Result<()> {
        self.sender
            .as_ref()
            .context("Editor is closed")?
            .try_send(Request::Frame(id, path, time))
            .map_err(|_| anyhow::anyhow!("The editor is busy. Try again shortly."))
    }
    pub fn filmstrip(&self, id: u64, path: PathBuf) -> Result<()> {
        self.sender
            .as_ref()
            .context("Editor is closed")?
            .try_send(Request::Filmstrip(id, path))
            .map_err(|_| anyhow::anyhow!("The editor is busy. Try again shortly."))
    }
    pub fn export(
        &self,
        project: EditProject,
        options: ExportOptions,
    ) -> Result<(Arc<Mutex<JobProgress>>, Arc<AtomicBool>)> {
        let state = Arc::new(Mutex::new(JobProgress {
            message: "Queued".into(),
            ..Default::default()
        }));
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancellations
            .lock()
            .unwrap()
            .push(Arc::downgrade(&cancel));
        self.export_sender
            .as_ref()
            .context("Editor is closed")?
            .try_send(Request::Export(
                project,
                options,
                state.clone(),
                cancel.clone(),
            ))
            .map_err(|_| {
                anyhow::anyhow!("One export is running and another is queued. Try again shortly.")
            })?;
        Ok((state, cancel))
    }
}
impl Drop for MediaService {
    fn drop(&mut self) {
        for token in self
            .cancellations
            .lock()
            .unwrap()
            .iter()
            .filter_map(std::sync::Weak::upgrade)
        {
            token.store(true, std::sync::atomic::Ordering::Release);
        }
        self.sender.take();
        self.export_sender.take();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}
