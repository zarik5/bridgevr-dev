// #![allow(
//     improper_ctypes,
//     non_snake_case,
//     non_camel_case_types,
//     non_upper_case_globals,
//     clippy::all
// )]

// use crate::video_encoder_interface::*;
// use bridgevr_common::*;
// use libloading::*;
// use std::ffi::c_void;
// use std::ptr::*;

// fn nvencapi_struct_version(ver: u32) -> u32 {
//     (NVENCAPI_VERSION | (ver << 16) | (0x7 << 28))
// }

// macro_rules! nv_struct {
//     ($struct_t:ident, $([no flag] $struct_version_number_no_flag:tt)? $([with flag] $struct_version_number_w_flag:tt)?) => {
//         $struct_t {
//             version: $(nvencapi_struct_version($struct_version_number_no_flag))?
//                      $(nvencapi_struct_version($struct_version_number_w_flag) | (1 << 31))?,
//             ..<_>::default()
//         }
//     }
// }

// macro_rules! nv_enc_success_or_panic {
//     ($($nv_call:ident).+($($params:tt)*)) => { unsafe {
//         let error_code = some_or_panic!($($nv_call).*, format!("Invalid function pointer: {}", stringify!($($nv_call).*)))($($params)*);
//         if error_code != NV_ENC_SUCCESS {
//             log_panic!(format!("NvEnc error ({}): {}", error_code, stringify!($($nv_call)*)));
//         }
//     }};
// }

// pub struct NvidiaEncoder {
//     module: Library,
//     nvenc_instance: NV_ENCODE_API_FUNCTION_LIST,
//     encoder_ptr: *mut c_void,
//     input_resource_ptr: *mut c_void,
//     output_bitstream_ptr: *mut c_void,
//     width: u32,
//     height: u32,
// }

// impl NvidiaEncoder {
//     pub fn new(
//         graphics_device_ptr: NonNull<c_void>,
//         width: u32,
//         height: u32,
//         frame_rate: u32,
//         codec: Codec,
//         input_texture: NonNull<c_void>, //DXGI_FORMAT_B8G8R8A8_UNORM!
//     ) -> Self {
//         let dyn_lib_name = if cfg!(windows) {
//             "nvEncodeAPI64.dll"
//         } else {
//             "nvcuvid.so"
//         };
//         let module = ok_or_panic!(Library::new(dyn_lib_name), "NvEnc library");
//         let mut system_version = 0u32;

//         unsafe {
//             let nv_err = ok_or_panic!(
//                 module.get::<fn(*mut u32) -> NVENCSTATUS>(b"NvEncodeAPIGetMaxSupportedVersion"),
//                 "Failed to call NvEncodeAPIGetMaxSupportedVersion"
//             )(&mut system_version);
//             if nv_err != NV_ENC_SUCCESS {
//                 log_panic!(format!(
//                     "NvEnc error ({}): {}",
//                     nv_err, "NvEncodeAPIGetMaxSupportedVersion"
//                 ));
//             }
//         }
//         if ((NVENCAPI_MAJOR_VERSION << 4) | NVENCAPI_MINOR_VERSION) > system_version {
//             log_panic!("NvEnc driver is too old");
//         }

//         let mut nvenc_instance = nv_struct!(NV_ENCODE_API_FUNCTION_LIST, [no flag] 2);

//         unsafe {
//             let nv_err = ok_or_panic!(
//                 module.get::<fn(*mut NV_ENCODE_API_FUNCTION_LIST) -> NVENCSTATUS>(
//                     b"NvEncodeAPICreateInstance"
//                 ),
//                 "Failed to call NvEncodeAPICreateInstance"
//             )(&mut nvenc_instance);
//             if nv_err != NV_ENC_SUCCESS {
//                 log_panic!(format!(
//                     "NvEnc error ({}): {}",
//                     nv_err, "NvEncodeAPICreateInstance"
//                 ));
//             }
//         }

//         let mut encode_session_params =
//             nv_struct!(NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS, [no flag] 1);
//         encode_session_params.apiVersion = NVENCAPI_VERSION;
//         encode_session_params.device = graphics_device_ptr.as_ptr();
//         encode_session_params.deviceType = if cfg!(windows) {
//             NV_ENC_DEVICE_TYPE_DIRECTX
//         } else {
//             NV_ENC_DEVICE_TYPE_CUDA
//         };

//         let mut encoder_ptr = null_mut();
//         nv_enc_success_or_panic!(nvenc_instance
//             .nvEncOpenEncodeSessionEx(&mut encode_session_params as _, &mut encoder_ptr as _));

//         let (codec_guid, input_preset, input_rc_params) = match &codec {
//             Codec::H264 {
//                 preset, rc_params, ..
//             } => (unsafe { NV_ENC_CODEC_H264_GUID }, preset, rc_params),
//             Codec::HEVC {
//                 preset, rc_params, ..
//             } => (unsafe { NV_ENC_CODEC_HEVC_GUID }, preset, rc_params),
//         };
//         let preset_guid = match input_preset {
//             Preset::Default => unsafe { NV_ENC_PRESET_DEFAULT_GUID },
//             Preset::HighPerformance => unsafe { NV_ENC_PRESET_HP_GUID },
//             Preset::HighQuality => unsafe { NV_ENC_PRESET_HQ_GUID },
//         };

//         let mut init_params = nv_struct!(NV_ENC_INITIALIZE_PARAMS, [with flag] 5);
//         init_params.encodeGUID = codec_guid;
//         init_params.presetGUID = preset_guid;
//         init_params.encodeWidth = width;
//         init_params.encodeHeight = height;
//         init_params.darWidth = width;
//         init_params.darHeight = height;
//         init_params.maxEncodeWidth = width;
//         init_params.maxEncodeHeight = height;
//         init_params.frameRateNum = frame_rate;
//         init_params.frameRateDen = 1;
//         init_params.enablePTD = 1; // enable forcing IDR frames

//         // if cfg!(windows) {
//         //     init_params.enableEncodeAsync = 1;
//         // }

//         let mut preset_config = nv_struct!(NV_ENC_PRESET_CONFIG, [with flag] 4);
//         preset_config.presetCfg = nv_struct!(NV_ENC_CONFIG, [with flag] 7);
//         nv_enc_success_or_panic!(nvenc_instance.nvEncGetEncodePresetConfig(
//             encoder_ptr,
//             codec_guid,
//             preset_guid,
//             &mut preset_config
//         ));
//         let mut encode_config = preset_config.presetCfg;
//         encode_config.frameIntervalP = 1; // set to 0 for intra frames only
//         encode_config.gopLength = NVENC_INFINITE_GOPLENGTH; // requires P frames. Leave infinite GOP, IDR will be inserted manually
//         let rc_params_ref = &mut encode_config.rcParams;
//         rc_params_ref.rateControlMode = match &input_rc_params.rate_control_mode {
//             RateControlMode::ConstantQP(maybe_qp_params) => {
//                 if let Some(qp) = maybe_qp_params {
//                     rc_params_ref.constQP = NV_ENC_QP {
//                         qpInterP: qp.p,
//                         qpInterB: 0,
//                         qpIntra: qp.i,
//                     };
//                 }
//                 NV_ENC_PARAMS_RC_CONSTQP
//             }
//             RateControlMode::VBR { .. } => NV_ENC_PARAMS_RC_VBR, // todo: bind vbr parameters
//             RateControlMode::CBR => NV_ENC_PARAMS_RC_CBR,
//             RateControlMode::LowDelayCBR => NV_ENC_PARAMS_RC_CBR_LOWDELAY_HQ,
//         };
//         if let Some(bitrate_k) = input_rc_params.bitrate_k {
//             rc_params_ref.averageBitRate = bitrate_k * 1000;
//         }
//         if let Some(vbv_buffer_size) = input_rc_params.vbv_buffer_size {
//             rc_params_ref.vbvBufferSize = vbv_buffer_size;
//         }
//         if let Some(vbv_initial_delay) = input_rc_params.vbv_initial_delay {
//             rc_params_ref.vbvInitialDelay = vbv_initial_delay;
//         }
//         if let Some(aq) = &input_rc_params.aq {
//             rc_params_ref._bitfield_1.set_bit(3, aq.enable_spatial);
//             rc_params_ref._bitfield_1.set_bit(8, aq.enable_temporal);
//             rc_params_ref._bitfield_1.set(12, 4, aq.strength as _);
//         };
//         if let Some(zero_latency) = input_rc_params.zero_latency {
//             rc_params_ref._bitfield_1.set_bit(9, zero_latency);
//         }
//         if let Some(enable_non_ref_p) = input_rc_params.enable_non_ref_p {
//             rc_params_ref._bitfield_1.set_bit(10, enable_non_ref_p);
//         }
//         if let Some(strict_gop_target) = input_rc_params.strict_gop_target {
//             rc_params_ref._bitfield_1.set_bit(11, strict_gop_target);
//         }

//         match &codec {
//             Codec::H264 { chroma_format, .. } => {
//                 unsafe { encode_config.encodeCodecConfig.h264Config }.chromaFormatIDC =
//                     match chroma_format.as_ref().unwrap_or(&ChromaFormat::YUV420) {
//                         ChromaFormat::YUV420 => 1,
//                         ChromaFormat::YUV444 => 3,
//                     };
//                 //todo: bind the rest of the parameters
//                 //todo check that idrPeriod is automatically set
//             }
//             Codec::HEVC { chroma_format, .. } => {
//                 // pixelBitDepthMinus8 already = 0,
//                 unsafe { encode_config.encodeCodecConfig.hevcConfig }
//                     ._bitfield_1
//                     .set(
//                         9,
//                         2,
//                         match chroma_format.as_ref().unwrap_or(&ChromaFormat::YUV420) {
//                             ChromaFormat::YUV420 => 1,
//                             ChromaFormat::YUV444 => 3,
//                         },
//                     );
//                 //todo: bind the rest of the parameters
//             }
//         }

//         init_params.encodeConfig = &mut encode_config;

//         nv_enc_success_or_panic!(
//             nvenc_instance.nvEncInitializeEncoder(encoder_ptr, &mut init_params)
//         );

//         // let completion_event = if cfg!(windows) {
//         //     let event_ptr = unsafe {
//         //         winapi::um::synchapi::CreateEventA(null_mut(), false as _, false as _, null_mut())
//         //     };
//         //     let mut event_params = nv_struct!(NV_ENC_EVENT_PARAMS, [no flag] 1);
//         //     event_params.completionEvent = event_ptr;
//         //     nv_enc_success_or_panic!(
//         //         nvenc_instance.nvEncRegisterAsyncEvent(encoder_ptr, &mut event_params)
//         //     );

//         //     Some(event_ptr)
//         // } else {
//         //     None
//         // };

//         let mut input_resource_params = nv_struct!(NV_ENC_REGISTER_RESOURCE, [no flag] 3);
//         input_resource_params.resourceType = if cfg!(windows) {
//             NV_ENC_INPUT_RESOURCE_TYPE_DIRECTX
//         } else {
//             NV_ENC_INPUT_RESOURCE_TYPE_OPENGL_TEX
//         };
//         input_resource_params.resourceToRegister = input_texture.as_ptr();
//         input_resource_params.width = width;
//         input_resource_params.height = height;
//         input_resource_params.pitch = 0;
//         input_resource_params.bufferFormat = NV_ENC_BUFFER_FORMAT_ARGB;
//         input_resource_params.bufferUsage = NV_ENC_INPUT_IMAGE;
//         nv_enc_success_or_panic!(
//             nvenc_instance.nvEncRegisterResource(encoder_ptr, &mut input_resource_params)
//         );

//         let mut output_bitstream_param = nv_struct!(NV_ENC_CREATE_BITSTREAM_BUFFER, [no flag] 1);
//         nv_enc_success_or_panic!(
//             nvenc_instance.nvEncCreateBitstreamBuffer(encoder_ptr, &mut output_bitstream_param)
//         );

//         Self {
//             module,
//             nvenc_instance,
//             encoder_ptr,
//             input_resource_ptr: input_resource_params.registeredResource,
//             output_bitstream_ptr: output_bitstream_param.bitstreamBuffer,
//             width,
//             height,
//         }
//     }
// }

// impl VideoEncoder for NvidiaEncoder {
//     //fn new() -> Self {
//     //    NvidiaEncoder {}
//     //}

//     fn encode(&mut self, force_idr: bool) {
//         let nvenc_instance = self.nvenc_instance;

//         let mut input_resource_params = nv_struct!(NV_ENC_MAP_INPUT_RESOURCE, [no flag] 4);
//         input_resource_params.registeredResource = self.input_resource_ptr;
//         nv_enc_success_or_panic!(
//             nvenc_instance.nvEncMapInputResource(self.encoder_ptr, &mut input_resource_params)
//         );
//         let mapped_input_resource_ptr = input_resource_params.mappedResource;
//         let mut pic_params = nv_struct!(NV_ENC_PIC_PARAMS, [with flag] 4);
//         pic_params.pictureStruct = NV_ENC_PIC_STRUCT_FRAME;
//         pic_params.inputBuffer = mapped_input_resource_ptr;
//         pic_params.bufferFmt = NV_ENC_BUFFER_FORMAT_ARGB;
//         pic_params.inputWidth = self.width;
//         pic_params.inputHeight = self.height;
//         pic_params.outputBitstream = self.output_bitstream_ptr;

//         nv_enc_success_or_panic!(
//             nvenc_instance.nvEncEncodePicture(self.encoder_ptr, &mut pic_params)
//         );
//         // (NV_ENC_ERR_NEED_MORE_INPUT should never happen)

//         std::unimplemented!();
//     }
// }

// // nvenc does not accept vulkan image as source
// // instead use: vulkan -> GL_NV_draw_vulkan_image -> nvenc
