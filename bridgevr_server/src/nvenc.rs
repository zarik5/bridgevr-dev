use bridgevr_common::{data::*, *};
use libloading::*;
use nvidia_video_codec_sdk_sys::*;
use std::ffi::c_void;
use std::ptr::*;

macro_rules! nv_trace_err {
    ($res:expr) => {{
        if $res == NV_ENC_SUCCESS {
            Ok(())
        } else {
            trace_err!(Err(format!("Code {}", $res)))
        }
    }};
}

macro_rules! nv_call_trace_err {
    ($($nv_call:ident).+($($params:tt)*)) => { unsafe {
        // unwrap should never panic
        nv_trace_err!($($nv_call).*.unwrap()($($params)*))
    }};
}

macro_rules! nv_struct {
    ($struct_t:ident) => {
        $struct_t {
            version: paste::expr!([< $struct_t _VERSION >]),
            ..<_>::default()
        }
    }
}

const TRACE_CONTEXT: &str = "NVENC";

pub struct NvidiaEncoder {
    module: Library,
    nvenc_instance: NV_ENCODE_API_FUNCTION_LIST,
    encoder_ptr: *mut c_void,
    input_resource_ptr: *mut c_void,
    output_bitstream_ptr: *mut c_void,
    width: u32,
    height: u32,
}

impl NvidiaEncoder {
    pub fn new(
        graphics_device_ptr: u64,
        width: u32,
        height: u32,
        frame_rate: u32,
        codec: NvCodecH264,
        input_texture: u64, //DXGI_FORMAT_B8G8R8A8_UNORM!
    ) -> StrResult<Self> {
        let dyn_lib_name = if cfg!(windows) {
            "nvEncodeAPI64.dll"
        } else if cfg!(target_os = "linux") {
            "nvcuvid.so"
        } else {
            return trace_str!("Unsupported OS");
        };
        let module = trace_err!(Library::new(dyn_lib_name), "NvEnc library")?;

        let mut system_version = 0u32;
        nv_trace_err!(trace_err!(unsafe {
            module.get::<fn(*mut u32) -> NVENCSTATUS>(b"NvEncodeAPIGetMaxSupportedVersion")
        })?(&mut system_version))?;

        if ((NVENCAPI_MAJOR_VERSION << 4) | NVENCAPI_MINOR_VERSION) > system_version {
            return trace_str!("NVENC driver is too old");
        }

        let mut nvenc_instance = nv_struct!(NV_ENCODE_API_FUNCTION_LIST);

        nv_trace_err!(trace_err!(unsafe {
            module.get::<fn(*mut NV_ENCODE_API_FUNCTION_LIST) -> NVENCSTATUS>(
                b"NvEncodeAPICreateInstance",
            )
        })?(&mut nvenc_instance))?;

        let mut encode_session_params = nv_struct!(NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS);
        encode_session_params.apiVersion = NVENCAPI_VERSION;
        encode_session_params.device = graphics_device_ptr as _;
        encode_session_params.deviceType = if cfg!(windows) {
            NV_ENC_DEVICE_TYPE_DIRECTX
        } else {
            NV_ENC_DEVICE_TYPE_CUDA
        };

        let mut encoder_ptr = null_mut();
        nv_call_trace_err!(nvenc_instance
            .nvEncOpenEncodeSessionEx(&mut encode_session_params as _, &mut encoder_ptr as _))?;

        let preset_guid = match codec.preset {
            Preset::Default => unsafe { NV_ENC_PRESET_DEFAULT_GUID },
            Preset::HighPerformance => unsafe { NV_ENC_PRESET_HP_GUID },
            Preset::HighQuality => unsafe { NV_ENC_PRESET_HQ_GUID },
        };

        let mut init_params = nv_struct!(NV_ENC_INITIALIZE_PARAMS);
        init_params.encodeGUID = unsafe { NV_ENC_CODEC_H264_GUID };
        init_params.presetGUID = preset_guid;
        init_params.encodeWidth = width;
        init_params.encodeHeight = height;
        init_params.darWidth = width;
        init_params.darHeight = height;
        init_params.maxEncodeWidth = width;
        init_params.maxEncodeHeight = height;
        init_params.frameRateNum = frame_rate;
        init_params.frameRateDen = 1;
        init_params.enablePTD = 1; // enable forcing IDR frames

        let mut preset_config = nv_struct!(NV_ENC_PRESET_CONFIG);
        preset_config.presetCfg = nv_struct!(NV_ENC_CONFIG);
        nv_call_trace_err!(nvenc_instance.nvEncGetEncodePresetConfig(
            encoder_ptr,
            NV_ENC_CODEC_H264_GUID,
            preset_guid,
            &mut preset_config
        ))?;
        let mut encode_config = preset_config.presetCfg;
        encode_config.frameIntervalP = 1; // set to 0 for intra frames only
        encode_config.gopLength = NVENC_INFINITE_GOPLENGTH; // requires P frames. Leave infinite GOP, IDR will be inserted manually
        let rc_params_ref = &mut encode_config.rcParams;
        if let Some(rc_mode) = codec.rate_control.mode {
            rc_params_ref.rateControlMode = match rc_mode {
                RateControlMode::ConstantQP(maybe_qp_params) => {
                    if let Some(qp) = maybe_qp_params {
                        rc_params_ref.constQP = NV_ENC_QP {
                            qpInterP: qp.p,
                            qpInterB: 0,
                            qpIntra: qp.i,
                        };
                    }
                    NV_ENC_PARAMS_RC_CONSTQP
                }
                RateControlMode::VBR { .. } => NV_ENC_PARAMS_RC_VBR, // todo: bind vbr parameters
                RateControlMode::CBR => NV_ENC_PARAMS_RC_CBR,
                RateControlMode::LowDelayCBR => NV_ENC_PARAMS_RC_CBR_LOWDELAY_HQ,
            };
        }
        if let Some(bitrate_k) = codec.rate_control.bitrate_k {
            rc_params_ref.averageBitRate = bitrate_k * 1000;
        }
        if let Some(vbv_buffer_size) = codec.rate_control.vbv_buffer_size {
            rc_params_ref.vbvBufferSize = vbv_buffer_size;
        }
        if let Some(vbv_initial_delay) = codec.rate_control.vbv_initial_delay {
            rc_params_ref.vbvInitialDelay = vbv_initial_delay;
        }
        if let Some(aq) = &codec.rate_control.aq {
            rc_params_ref._bitfield_1.set_bit(3, aq.enable_spatial);
            rc_params_ref._bitfield_1.set_bit(8, aq.enable_temporal);
            rc_params_ref._bitfield_1.set(12, 4, aq.strength as _);
        };
        if let Some(zero_latency) = codec.rate_control.zero_latency {
            rc_params_ref._bitfield_1.set_bit(9, zero_latency);
        }
        if let Some(enable_non_ref_p) = codec.rate_control.enable_non_ref_p {
            rc_params_ref._bitfield_1.set_bit(10, enable_non_ref_p);
        }
        if let Some(strict_gop_target) = codec.rate_control.strict_gop_target {
            rc_params_ref._bitfield_1.set_bit(11, strict_gop_target);
        }

        unsafe { encode_config.encodeCodecConfig.h264Config }.chromaFormatIDC = match codec
            .chroma_format
            .unwrap_or(ChromaFormat::YUV420)
        {
            ChromaFormat::YUV420 => 1,
            ChromaFormat::YUV444 => 3,
        };
        //todo: bind the rest of the parameters
        //todo check that idrPeriod is automatically set

        init_params.encodeConfig = &mut encode_config;

        nv_call_trace_err!(nvenc_instance.nvEncInitializeEncoder(encoder_ptr, &mut init_params))?;

        let mut input_resource_params = nv_struct!(NV_ENC_REGISTER_RESOURCE);
        input_resource_params.resourceType = if cfg!(windows) {
            NV_ENC_INPUT_RESOURCE_TYPE_DIRECTX
        } else {
            NV_ENC_INPUT_RESOURCE_TYPE_CUDAARRAY
        };
        input_resource_params.resourceToRegister = input_texture as _;
        input_resource_params.width = width;
        input_resource_params.height = height;
        input_resource_params.pitch = 0;
        input_resource_params.bufferFormat = NV_ENC_BUFFER_FORMAT_ARGB;
        input_resource_params.bufferUsage = NV_ENC_INPUT_IMAGE;
        nv_call_trace_err!(
            nvenc_instance.nvEncRegisterResource(encoder_ptr, &mut input_resource_params)
        )?;

        let mut output_bitstream_param = nv_struct!(NV_ENC_CREATE_BITSTREAM_BUFFER);
        nv_call_trace_err!(
            nvenc_instance.nvEncCreateBitstreamBuffer(encoder_ptr, &mut output_bitstream_param)
        )?;

        Ok(Self {
            module,
            nvenc_instance,
            encoder_ptr,
            input_resource_ptr: input_resource_params.registeredResource,
            output_bitstream_ptr: output_bitstream_param.bitstreamBuffer,
            width,
            height,
        })
    }

    fn encode(&self, force_idr: bool) -> StrResult<()> {
        let nvenc_instance = self.nvenc_instance;

        let mut input_resource_params = nv_struct!(NV_ENC_MAP_INPUT_RESOURCE);
        input_resource_params.registeredResource = self.input_resource_ptr;
        nv_call_trace_err!(
            nvenc_instance.nvEncMapInputResource(self.encoder_ptr, &mut input_resource_params)
        )?;
        let mapped_input_resource_ptr = input_resource_params.mappedResource;
        let mut pic_params = nv_struct!(NV_ENC_PIC_PARAMS);
        pic_params.pictureStruct = NV_ENC_PIC_STRUCT_FRAME;
        pic_params.inputBuffer = mapped_input_resource_ptr;
        pic_params.bufferFmt = NV_ENC_BUFFER_FORMAT_ARGB;
        pic_params.inputWidth = self.width;
        pic_params.inputHeight = self.height;
        pic_params.outputBitstream = self.output_bitstream_ptr;

        nv_call_trace_err!(nvenc_instance.nvEncEncodePicture(self.encoder_ptr, &mut pic_params))?;
        // (NV_ENC_ERR_NEED_MORE_INPUT should never happen)

        todo!();
    }
}

// nvenc does not accept directly a vulkan image as source
// see:
// https://devtalk.nvidia.com/default/topic/1045258/video-codec-and-optical-flow-sdk/use-video-codec-sdk-to-encode-vulkan-images/
// https://stackoverflow.com/questions/55424875/use-vulkan-vkimage-as-a-cuda-cuarray