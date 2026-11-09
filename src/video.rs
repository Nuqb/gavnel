mod converter;
mod session;
use crate::replay::{Packet, Stream, TICKS, VideoConfig, annex_b_nals, avcc};
use anyhow::{Context, Result, bail};
use converter::Converter;
use crossbeam_channel::{Receiver, Sender, bounded};
use mini_nvcodec::{NvencApiVersion, guids::*, loader::NvencApi, nvenc_sys::*};
use session::Session;
use std::{
    ffi::CStr,
    mem::ManuallyDrop,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    thread::JoinHandle,
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, RECT, WAIT_OBJECT_0},
        Graphics::{
            Direct3D11::*,
            Dxgi::{Common::*, IDXGIDevice},
        },
        System::Threading::{CreateEventW, WaitForSingleObject},
    },
    core::Interface,
};

// Every FFI struct in the NVENC API explicitly permits zero-initialization.
fn zeroed<T>() -> T {
    unsafe { std::mem::zeroed() }
}
fn status(code: NVENCSTATUS, operation: &str) -> Result<()> {
    if code != NVENCSTATUS::SUCCESS {
        bail!(
            "NVIDIA {operation} failed ({code:?}). Update the NVIDIA driver or close other recording apps."
        );
    }
    Ok(())
}
pub fn driver_available() -> bool {
    mini_nvcodec::try_init_nvenc().is_some()
}

struct Job {
    index: usize,
    mapped: usize,
}
pub struct VideoEncoder {
    session: Arc<Session>,
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    converter: Option<Converter>,
    black: ID3D11Texture2D,
    free: Receiver<usize>,
    free_tx: Sender<usize>,
    jobs: Option<Sender<Job>>,
    worker: Option<JoinHandle<()>>,
    width: u32,
    height: u32,
    fps: u32,
    frame_index: u32,
    pub config: VideoConfig,
    pub dropped: Arc<AtomicU64>,
}
impl VideoEncoder {
    pub fn new(
        device: ID3D11Device,
        context: ID3D11DeviceContext,
        settings: &crate::config::Settings,
        output: crate::replay::EncoderOutput,
    ) -> Result<Self> {
        let crate::replay::EncoderOutput {
            budget,
            packets,
            messages: errors,
        } = output;
        let (height, fps, mbps) = (settings.height, settings.fps, settings.bitrate_mbps);
        let width = (height * 16 / 9) & !1;
        let (session, config) = Session::new(
            &device,
            session::Format {
                width,
                height,
                fps,
                mbps,
            },
        )?;
        let session = Arc::new(session);
        let mut black = None;
        let pixels = [0u32; 16 * 16];
        let desc = D3D11_TEXTURE2D_DESC {
            Width: 16,
            Height: 16,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
            ..Default::default()
        };
        let data = D3D11_SUBRESOURCE_DATA {
            pSysMem: pixels.as_ptr() as _,
            SysMemPitch: 16 * 4,
            SysMemSlicePitch: 0,
        };
        unsafe {
            device.CreateTexture2D(&desc, Some(&data), Some(&mut black))?;
        }
        let (free_tx, free) = bounded(4);
        for i in 0..4 {
            free_tx.send(i)?;
        }
        let (jobs_tx, jobs_rx) = bounded::<Job>(4);
        let dropped = Arc::new(AtomicU64::new(0));
        let worker_session = session.clone();
        let worker_free = free_tx.clone();
        let lost = dropped.clone();
        let worker=std::thread::Builder::new().name("nvenc-completion".into()).spawn(move||{
            let s=worker_session;
            for job in jobs_rx {
                let slot=&s.slots[job.index];
                let result=(||->Result<()>{unsafe{
                    if WaitForSingleObject(HANDLE(slot.event as _),5000)!=WAIT_OBJECT_0 {bail!("The NVIDIA encoder stopped responding. Stop capture and update the display driver.");}
                    let mut lock:NV_ENC_LOCK_BITSTREAM=zeroed();lock.version=s.version.lock_bitstream_ver();lock.outputBitstream=slot.output as _;
                    s.check((s.api.nvEncLockBitstream)(s.ptr(),&mut lock),"bitstream read")?;
                    let encoded=std::slice::from_raw_parts(lock.bitstreamBufferPtr as *const u8,lock.bitstreamSizeInBytes as usize);
                    let converted=avcc(encoded);
                    let _=(s.api.nvEncUnlockBitstream)(s.ptr(),slot.output as _);
                    let _=(s.api.nvEncUnmapInputResource)(s.ptr(),job.mapped as _);
                    let bytes=converted?;
                    if let Some(data)=budget.allocate(bytes) {
                        let packet=Packet{stream:Stream::Video,pts:lock.outputTimeStamp,duration:TICKS/fps as u64,keyframe:lock.pictureType==NV_ENC_PIC_TYPE::IDR,data};
                        if packets.try_send(packet).is_err(){lost.fetch_add(1,Ordering::Relaxed);}
                    }else{lost.fetch_add(1,Ordering::Relaxed);}
                    Ok(())
                }})();
                if let Err(e)=result {let _=errors.try_send(format!("{e:#}"));break;}
                if worker_free.send(job.index).is_err(){break;}
            }
        })?;
        Ok(Self {
            session,
            device,
            context,
            converter: None,
            black: black.unwrap(),
            free,
            free_tx,
            jobs: Some(jobs_tx),
            worker: Some(worker),
            width,
            height,
            fps,
            frame_index: 0,
            config,
            dropped,
        })
    }
    pub fn submit(&mut self, input: Option<&ID3D11Texture2D>, pts: u64) -> Result<bool> {
        let Ok(index) = self.free.try_recv() else {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return Ok(false);
        };
        let result = (|| -> Result<()> {
            unsafe {
                let input = input.unwrap_or(&self.black);
                let mut desc = D3D11_TEXTURE2D_DESC::default();
                input.GetDesc(&mut desc);
                if desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM
                    && desc.Format != DXGI_FORMAT_R8G8B8A8_UNORM
                {
                    bail!(
                        "HDR capture is not supported yet. Turn off HDR for this display to record SDR clips."
                    );
                }
                let size = (desc.Width, desc.Height);
                if self.converter.as_ref().is_none_or(|c| c.input_size != size) {
                    self.converter = Some(Converter::new(
                        &self.device,
                        &self.context,
                        size,
                        (self.width, self.height),
                        self.fps,
                    )?);
                }
                self.converter.as_ref().unwrap().convert(
                    input,
                    &self.session.slots[index].texture,
                    self.frame_index,
                )?;
                let s = &self.session;
                let slot = &s.slots[index];
                let mut map: NV_ENC_MAP_INPUT_RESOURCE = zeroed();
                map.version = s.version.map_input_resource_ver();
                map.registeredResource = slot.registered as _;
                s.check(
                    (s.api.nvEncMapInputResource)(s.ptr(), &mut map),
                    "GPU resource mapping",
                )?;
                let mut pic: NV_ENC_PIC_PARAMS = zeroed();
                pic.version = s.version.pic_params_ver();
                pic.inputWidth = self.width;
                pic.inputHeight = self.height;
                pic.inputBuffer = map.mappedResource;
                pic.outputBitstream = slot.output as _;
                pic.completionEvent = slot.event as _;
                pic.bufferFmt = NV_ENC_BUFFER_FORMAT::NV12;
                pic.pictureStruct = NV_ENC_PIC_STRUCT::FRAME;
                pic.inputTimeStamp = pts;
                pic.inputDuration = TICKS / self.fps as u64;
                pic.frameIdx = self.frame_index;
                if self.frame_index.is_multiple_of(self.fps) {
                    pic.encodePicFlags = NV_ENC_PIC_FLAG_FORCEIDR | NV_ENC_PIC_FLAG_OUTPUT_SPSPPS;
                }
                let code = (s.api.nvEncEncodePicture)(s.ptr(), &mut pic);
                if code != NVENCSTATUS::SUCCESS && code != NVENCSTATUS::ERR_NEED_MORE_INPUT {
                    let _ = (s.api.nvEncUnmapInputResource)(s.ptr(), map.mappedResource);
                    s.check(code, "frame submission")?;
                }
                self.jobs
                    .as_ref()
                    .unwrap()
                    .try_send(Job {
                        index,
                        mapped: map.mappedResource as usize,
                    })
                    .context("NVIDIA completion worker stopped")?;
                self.frame_index = self.frame_index.wrapping_add(1);
                Ok(())
            }
        })();
        if result.is_err() {
            let _ = self.free_tx.try_send(index);
        }
        result.map(|()| true)
    }
}
impl Drop for VideoEncoder {
    fn drop(&mut self) {
        unsafe {
            let mut eos: NV_ENC_PIC_PARAMS = zeroed();
            eos.version = self.session.version.pic_params_ver();
            eos.encodePicFlags = NV_ENC_PIC_FLAG_EOS;
            let _ = (self.session.api.nvEncEncodePicture)(self.session.ptr(), &mut eos);
        }
        self.jobs.take();
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}
