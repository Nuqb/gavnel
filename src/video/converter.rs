//! Direct3D video conversion into encoder input textures.
use super::*;
pub(super) struct Converter {
    pub(super) device: ID3D11VideoDevice,
    pub(super) context: ID3D11VideoContext,
    pub(super) enumerator: ID3D11VideoProcessorEnumerator,
    pub(super) processor: ID3D11VideoProcessor,
    pub(super) input_size: (u32, u32),
}
impl Converter {
    pub(super) fn new(
        device: &ID3D11Device,
        context: &ID3D11DeviceContext,
        input: (u32, u32),
        output: (u32, u32),
        fps: u32,
    ) -> Result<Self> {
        unsafe {
            let device: ID3D11VideoDevice = device.cast()?;
            let context: ID3D11VideoContext = context.cast()?;
            let rate = DXGI_RATIONAL {
                Numerator: fps,
                Denominator: 1,
            };
            let desc = D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
                InputFrameFormat: D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
                InputFrameRate: rate,
                InputWidth: input.0,
                InputHeight: input.1,
                OutputFrameRate: rate,
                OutputWidth: output.0,
                OutputHeight: output.1,
                Usage: D3D11_VIDEO_USAGE_PLAYBACK_NORMAL,
            };
            let enumerator = device.CreateVideoProcessorEnumerator(&desc)?;
            let processor = device.CreateVideoProcessor(&enumerator, 0)?;
            context.VideoProcessorSetStreamAutoProcessingMode(&processor, 0, false);
            let full = RECT {
                left: 0,
                top: 0,
                right: output.0 as i32,
                bottom: output.1 as i32,
            };
            context.VideoProcessorSetOutputTargetRect(&processor, true, Some(&full));
            let scale = (output.0 as f64 / input.0 as f64).min(output.1 as f64 / input.1 as f64);
            let w = (input.0 as f64 * scale) as i32 & !1;
            let h = (input.1 as f64 * scale) as i32 & !1;
            let x = (output.0 as i32 - w) / 2;
            let y = (output.1 as i32 - h) / 2;
            let dest = RECT {
                left: x,
                top: y,
                right: x + w,
                bottom: y + h,
            };
            context.VideoProcessorSetStreamDestRect(&processor, 0, true, Some(&dest));
            let color = D3D11_VIDEO_COLOR {
                Anonymous: D3D11_VIDEO_COLOR_0 {
                    RGBA: D3D11_VIDEO_COLOR_RGBA {
                        R: 0.0,
                        G: 0.0,
                        B: 0.0,
                        A: 1.0,
                    },
                },
            };
            context.VideoProcessorSetOutputBackgroundColor(&processor, false, &color);
            // Input RGB full-range; output limited-range BT.709 YCbCr.
            let input_color = D3D11_VIDEO_PROCESSOR_COLOR_SPACE { _bitfield: 0 };
            let output_color = D3D11_VIDEO_PROCESSOR_COLOR_SPACE { _bitfield: 2 };
            context.VideoProcessorSetStreamColorSpace(&processor, 0, &input_color);
            context.VideoProcessorSetOutputColorSpace(&processor, &output_color);
            Ok(Self {
                device,
                context,
                enumerator,
                processor,
                input_size: input,
            })
        }
    }
    pub(super) fn convert(
        &self,
        input: &ID3D11Texture2D,
        output: &ID3D11Texture2D,
        frame_index: u32,
    ) -> Result<()> {
        unsafe {
            let desc = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC {
                FourCC: 0,
                ViewDimension: D3D11_VPIV_DIMENSION_TEXTURE2D,
                ..Default::default()
            };
            let mut iv = None;
            self.device.CreateVideoProcessorInputView(
                input,
                &self.enumerator,
                &desc,
                Some(&mut iv),
            )?;
            let desc = D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
                ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2D,
                ..Default::default()
            };
            let mut ov = None;
            self.device.CreateVideoProcessorOutputView(
                output,
                &self.enumerator,
                &desc,
                Some(&mut ov),
            )?;
            let mut stream = D3D11_VIDEO_PROCESSOR_STREAM {
                Enable: true.into(),
                InputFrameOrField: frame_index,
                pInputSurface: ManuallyDrop::new(iv),
                ..Default::default()
            };
            let result = self.context.VideoProcessorBlt(
                &self.processor,
                ov.as_ref().unwrap(),
                frame_index,
                std::slice::from_ref(&stream),
            );
            ManuallyDrop::drop(&mut stream.pInputSurface);
            result?;
            Ok(())
        }
    }
}
