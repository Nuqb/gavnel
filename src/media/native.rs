//! In-process Windows media services. No helper executables or bundled codecs.
use anyhow::{Context, Result, ensure};
use std::path::Path;
use windows::{
    Win32::{
        Media::MediaFoundation::*,
        System::{Com::StructuredStorage::*, Com::*, Variant::VT_I8},
    },
    core::{GUID, PCWSTR},
};

pub(super) const TICKS: f64 = 10_000_000.0;

pub(super) struct Runtime {
    com: bool,
}
impl Runtime {
    pub fn new() -> Result<Self> {
        unsafe {
            let initialized = CoInitializeEx(None, COINIT_MULTITHREADED);
            if initialized.is_err() && initialized.0 as u32 != 0x80010106 {
                initialized.ok()?;
            }
            let com = initialized.is_ok();
            if let Err(error) = MFStartup(MF_VERSION, MFSTARTUP_FULL) {
                if com {
                    CoUninitialize();
                }
                return Err(error).context("Windows media services are unavailable.");
            }
            Ok(Self { com })
        }
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
            if self.com {
                CoUninitialize();
            }
        }
    }
}

pub(super) fn wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    let mut path: Vec<u16> = path.as_os_str().encode_wide().collect();
    // Media Foundation URL APIs need ordinary Win32 paths. Preserve Unicode
    // and UNC server names when removing canonicalize's extended prefix.
    if path.starts_with(&[92, 92, 63, 92]) {
        path.drain(..4);
        if path.starts_with(&[85, 78, 67, 92]) {
            path.splice(..4, [92, 92]);
        }
    }
    path.push(0);
    path
}

pub(super) fn media_type(major: &GUID, subtype: &GUID) -> Result<IMFMediaType> {
    unsafe {
        let media = MFCreateMediaType()?;
        media.SetGUID(&MF_MT_MAJOR_TYPE, major)?;
        media.SetGUID(&MF_MT_SUBTYPE, subtype)?;
        Ok(media)
    }
}

pub(super) struct Source {
    pub reader: IMFSourceReader,
    _runtime: Runtime,
}
impl Source {
    pub fn open(path: &Path) -> Result<Self> {
        let runtime = Runtime::new()?;
        let path = std::path::absolute(path).context("Video path is unavailable.")?;
        unsafe {
            let mut attributes = None;
            MFCreateAttributes(&mut attributes, 1)?;
            let attributes = attributes.unwrap();
            attributes.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)?;
            let reader = MFCreateSourceReaderFromURL(PCWSTR(wide(&path).as_ptr()), &attributes)
                .with_context(|| format!("Windows could not open video: {}", path.display()))?;
            Ok(Self {
                reader,
                _runtime: runtime,
            })
        }
    }
    pub fn select(&self, index: u32, media: &IMFMediaType) -> Result<()> {
        unsafe {
            self.reader
                .SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
            self.reader.SetStreamSelection(index, true)?;
            self.reader.SetCurrentMediaType(index, None, media)?;
        }
        Ok(())
    }
    pub fn seek(&self, seconds: f64) -> Result<()> {
        let mut position = PROPVARIANT::default();
        unsafe {
            (*position.Anonymous.Anonymous).vt = VT_I8;
            (*position.Anonymous.Anonymous).Anonymous.hVal =
                (seconds.max(0.0) * TICKS).round() as i64;
            self.reader.SetCurrentPosition(&GUID::zeroed(), &position)?;
        }
        Ok(())
    }
    pub fn read(&self, index: u32) -> Result<Option<(i64, IMFSample)>> {
        for _ in 0..1024 {
            let (mut flags, mut timestamp, mut sample) = (0, 0, None);
            unsafe {
                self.reader.ReadSample(
                    index,
                    0,
                    None,
                    Some(&mut flags),
                    Some(&mut timestamp),
                    Some(&mut sample),
                )?;
            }
            ensure!(
                flags & MF_SOURCE_READERF_ERROR.0 as u32 == 0,
                "Windows could not decode this video."
            );
            if let Some(sample) = sample {
                return Ok(Some((timestamp, sample)));
            }
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                return Ok(None);
            }
        }
        anyhow::bail!("Windows returned no decoded media samples.")
    }
}

pub(super) fn sample_bytes(sample: &IMFSample) -> Result<Vec<u8>> {
    unsafe {
        let buffer = sample.ConvertToContiguousBuffer()?;
        let (mut pointer, mut length) = (std::ptr::null_mut(), 0);
        buffer.Lock(&mut pointer, None, Some(&mut length))?;
        let bytes = std::slice::from_raw_parts(pointer, length as usize).to_vec();
        buffer.Unlock()?;
        Ok(bytes)
    }
}

pub(super) fn make_sample(bytes: &[u8], time: i64, duration: i64) -> Result<IMFSample> {
    unsafe {
        let sample = MFCreateSample()?;
        let buffer = MFCreateMemoryBuffer(u32::try_from(bytes.len())?)?;
        let mut pointer = std::ptr::null_mut();
        buffer.Lock(&mut pointer, None, None)?;
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer, bytes.len());
        buffer.Unlock()?;
        buffer.SetCurrentLength(bytes.len() as u32)?;
        sample.AddBuffer(&buffer)?;
        sample.SetSampleTime(time)?;
        sample.SetSampleDuration(duration)?;
        Ok(sample)
    }
}
