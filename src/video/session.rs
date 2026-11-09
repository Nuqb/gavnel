//! NVIDIA encoder session, registered slots, and native resource cleanup.
use super::*;
pub(super) struct Format {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub mbps: u32,
}
pub(super) struct Slot {
    pub(super) texture: ID3D11Texture2D,
    pub(super) registered: usize,
    pub(super) output: usize,
    pub(super) event: usize,
}
pub(super) struct Session {
    pub(super) api: &'static NvencApi,
    pub(super) version: NvencApiVersion,
    pub(super) handle: usize,
    pub(super) slots: Vec<Slot>,
}
// Session calls use NVIDIA's supported asynchronous submission/completion model.
// Slot textures are reused only after the completion worker returns their index.
// The shared Direct3D immediate context is multithread-protected.
unsafe impl Send for Session {}
unsafe impl Sync for Session {}
impl Session {
    pub(super) fn ptr(&self) -> *mut std::ffi::c_void {
        self.handle as _
    }
    pub(super) fn check(&self, code: NVENCSTATUS, operation: &str) -> Result<()> {
        if code == NVENCSTATUS::SUCCESS {
            return Ok(());
        }
        let p = unsafe { (self.api.nvEncGetLastErrorString)(self.ptr()) };
        let detail = if p.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
        };
        bail!("NVIDIA {operation} failed ({code:?}): {detail}");
    }
    pub(super) fn new(device: &ID3D11Device, format: Format) -> Result<(Self, VideoConfig)> {
        let Format {
            width,
            height,
            fps,
            mbps,
        } = format;
        unsafe {
            let desc = device.cast::<IDXGIDevice>()?.GetAdapter()?.GetDesc()?;
            if desc.VendorId != 0x10de {
                bail!(
                    "The selected display is on a non-NVIDIA graphics adapter. Connect it to the RTX GPU; Gavnel does not fall back to CPU encoding."
                );
            }
            let api=mini_nvcodec::try_init_nvenc().ok_or_else(||anyhow::anyhow!("NVIDIA hardware encoding is unavailable. Install or update the NVIDIA display driver."))?;
            let version = api.created_with;
            let mut open: NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS = zeroed();
            open.version = version.open_encode_session_ex_params_ver();
            open.deviceType = NV_ENC_DEVICE_TYPE::DIRECTX;
            open.device = device.as_raw();
            open.apiVersion = version.nvencapi_version();
            let mut handle = std::ptr::null_mut();
            status(
                (api.nvEncOpenEncodeSessionEx)(&mut open, &mut handle),
                "session creation",
            )?;
            let mut s = Session {
                api,
                version,
                handle: handle as usize,
                slots: Vec::new(),
            };
            let mut preset: NV_ENC_PRESET_CONFIG = zeroed();
            preset.version = version.preset_config_ver();
            preset.presetCfg.version = version.config_ver();
            s.check(
                (api.nvEncGetEncodePresetConfigEx)(
                    handle,
                    NV_ENC_CODEC_H264_GUID,
                    NV_ENC_PRESET_P3_GUID,
                    NV_ENC_TUNING_INFO::LOW_LATENCY,
                    &mut preset,
                ),
                "preset selection",
            )?;
            let mut config = preset.presetCfg;
            config.profileGUID = NV_ENC_H264_PROFILE_HIGH_GUID;
            config.gopLength = fps;
            config.frameIntervalP = 1;
            config.rcParams.rateControlMode = NV_ENC_PARAMS_RC_MODE::CBR;
            config.rcParams.averageBitRate = mbps * 1_000_000;
            config.rcParams.maxBitRate = mbps * 1_000_000;
            config.rcParams.vbvBufferSize = mbps * 1_000_000;
            config.rcParams.vbvInitialDelay = mbps * 1_000_000;
            config.rcParams.multiPass = NV_ENC_MULTI_PASS::DISABLED;
            config.rcParams.set_enableAQ(0);
            config.rcParams.set_enableTemporalAQ(0);
            config.rcParams.set_enableLookahead(0);
            config.rcParams.lookaheadDepth = 0;
            config.encodeCodecConfig.h264Config.idrPeriod = fps;
            config.encodeCodecConfig.h264Config.set_repeatSPSPPS(1);
            config
                .encodeCodecConfig
                .h264Config
                .h264VUIParameters
                .videoSignalTypePresentFlag = 1;
            config
                .encodeCodecConfig
                .h264Config
                .h264VUIParameters
                .colourDescriptionPresentFlag = 1;
            config
                .encodeCodecConfig
                .h264Config
                .h264VUIParameters
                .colourPrimaries = NV_ENC_VUI_COLOR_PRIMARIES::BT709;
            config
                .encodeCodecConfig
                .h264Config
                .h264VUIParameters
                .transferCharacteristics = NV_ENC_VUI_TRANSFER_CHARACTERISTIC::BT709;
            config
                .encodeCodecConfig
                .h264Config
                .h264VUIParameters
                .colourMatrix = NV_ENC_VUI_MATRIX_COEFFS::BT709;
            let mut init: NV_ENC_INITIALIZE_PARAMS = zeroed();
            init.version = version.initialize_params_ver();
            init.encodeGUID = NV_ENC_CODEC_H264_GUID;
            init.presetGUID = NV_ENC_PRESET_P3_GUID;
            init.encodeWidth = width;
            init.encodeHeight = height;
            init.darWidth = width;
            init.darHeight = height;
            init.frameRateNum = fps;
            init.frameRateDen = 1;
            init.enableEncodeAsync = 1;
            init.enablePTD = 1;
            init.tuningInfo = NV_ENC_TUNING_INFO::LOW_LATENCY;
            init.encodeConfig = &mut config;
            s.check(
                (api.nvEncInitializeEncoder)(handle, &mut init),
                "encoder initialization",
            )?;
            let mut headers = vec![0u8; 4096];
            let mut header_size = 0u32;
            let mut seq: NV_ENC_SEQUENCE_PARAM_PAYLOAD = zeroed();
            seq.version = version.sequence_param_payload_ver();
            seq.inBufferSize = headers.len() as u32;
            seq.spsppsBuffer = headers.as_mut_ptr() as _;
            seq.outSPSPPSPayloadSize = &mut header_size;
            s.check(
                (api.nvEncGetSequenceParams)(handle, &mut seq),
                "H.264 headers",
            )?;
            let nals = annex_b_nals(&headers[..header_size as usize]);
            let sps = nals
                .iter()
                .find(|n| n[0] & 31 == 7)
                .context("NVIDIA did not return H.264 SPS")?
                .to_vec();
            let pps = nals
                .iter()
                .find(|n| n[0] & 31 == 8)
                .context("NVIDIA did not return H.264 PPS")?
                .to_vec();
            for _ in 0..4 {
                let desc = D3D11_TEXTURE2D_DESC {
                    Width: width,
                    Height: height,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: DXGI_FORMAT_NV12,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
                    ..Default::default()
                };
                let mut texture = None;
                device.CreateTexture2D(&desc, None, Some(&mut texture))?;
                let texture = texture.unwrap();
                let mut reg: NV_ENC_REGISTER_RESOURCE = zeroed();
                reg.version = version.register_resource_ver();
                reg.resourceType = NV_ENC_INPUT_RESOURCE_TYPE::DIRECTX;
                reg.width = width;
                reg.height = height;
                reg.resourceToRegister = texture.as_raw();
                reg.bufferFormat = NV_ENC_BUFFER_FORMAT::NV12;
                reg.bufferUsage = NV_ENC_BUFFER_USAGE::INPUT_IMAGE;
                s.check(
                    (api.nvEncRegisterResource)(handle, &mut reg),
                    "GPU texture registration",
                )?;
                s.slots.push(Slot {
                    texture,
                    registered: reg.registeredResource as usize,
                    output: 0,
                    event: 0,
                });
                let index = s.slots.len() - 1;
                let mut output: NV_ENC_CREATE_BITSTREAM_BUFFER = zeroed();
                output.version = version.create_bitstream_buffer_ver();
                s.check(
                    (api.nvEncCreateBitstreamBuffer)(handle, &mut output),
                    "bitstream allocation",
                )?;
                s.slots[index].output = output.bitstreamBuffer as usize;
                let event = CreateEventW(None, false, false, None)?;
                s.slots[index].event = event.0 as usize;
                let mut params: NV_ENC_EVENT_PARAMS = zeroed();
                params.version = version.event_params_ver();
                params.completionEvent = event.0;
                s.check(
                    (api.nvEncRegisterAsyncEvent)(handle, &mut params),
                    "async event registration",
                )?;
            }
            Ok((
                s,
                VideoConfig {
                    width: width as u16,
                    height: height as u16,
                    sps,
                    pps,
                },
            ))
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        unsafe {
            for slot in &self.slots {
                if slot.event != 0 {
                    let mut e: NV_ENC_EVENT_PARAMS = zeroed();
                    e.version = self.version.event_params_ver();
                    e.completionEvent = slot.event as _;
                    let _ = (self.api.nvEncUnregisterAsyncEvent)(self.ptr(), &mut e);
                    let _ = CloseHandle(HANDLE(slot.event as _));
                }
                if slot.output != 0 {
                    let _ = (self.api.nvEncDestroyBitstreamBuffer)(self.ptr(), slot.output as _);
                }
                if slot.registered != 0 {
                    let _ = (self.api.nvEncUnregisterResource)(self.ptr(), slot.registered as _);
                }
            }
            let _ = (self.api.nvEncDestroyEncoder)(self.ptr());
        }
    }
}
