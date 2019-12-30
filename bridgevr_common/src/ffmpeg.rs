#![allow(clippy::cast_ptr_alignment)]

use crate::{data::*, rendering::*, *};
use cuda::{driver::*, ffi::cuda::*, runtime::*};
use ffmpeg_sys::*;
use std::{ffi::CString, mem::transmute, os::raw::*, ptr::null_mut, sync::Arc};

//https://stackoverflow.com/questions/49862610/opengl-to-ffmpeg-encode
// convert both vulkan and directx texures to cuda array and then copy to ffmpeg cuda array

const TRACE_CONTEXT: &str = "FFmpeg";

const PIXEL_FORMAT: AVPixelFormat = AV_PIX_FMT_RGB32;

macro_rules! trace_av_err {
    ($res:expr $(, $expect_fmt:expr $(, $args:expr)*)?) => {{
        if $res != 0  {
            trace_err!(Err($res) $(, $expect_fmt $(, $args)*)?)
        } else {
            Ok(())
        }
    }};
}

macro_rules! trace_cu_err {
    ($res:expr $(, $expect_fmt:expr $(, $args:expr)*)?) => { unsafe {
        if $res != 0  {
            trace_err!(Err($res) $(, $expect_fmt $(, $args)*)?)
        } else {
            Ok(())
        }
    }};
}

macro_rules! trace_null_ptr {
    ($ptr:expr $(, $expect_fmt:expr $(, $args:expr)*)?) => { unsafe {
        if $ptr.is_null() {
            trace_str!("{}", String::new() $(+ &format!($expect_fmt $(, $args)*))?)
        } else {
            Ok($ptr)
        }
    }};
}

// patch for incomplete ffmpeg bindings:
#[repr(C)]
struct AVCUDADeviceContext {
    cuda_ctx: CUcontext,
    stream: CUstream,
    // ignore other fields
}

// WARNING: returned pointer must not be used after `string` drop.
unsafe fn to_c_str(string: &str) -> StrResult<*const c_char> {
    Ok(trace_err!(CString::new(string))?.as_ptr())
}

fn alloc_and_fill_av_dict(entries: Vec<(String, String)>) -> StrResult<*mut AVDictionary> {
    // separate c strings creation from `as_ptr` call to avoid dropping the c
    // strings
    let mut c_str_entries = vec![];
    for (key, value) in entries {
        let key_c_str = trace_err!(CString::new(key))?;
        let value_c_str = trace_err!(CString::new(value))?;
        c_str_entries.push((key_c_str, value_c_str))
    }

    let mut dict_ptr = null_mut(); // dict is allocated on first `av_dict_set` call.
    for (key, value) in c_str_entries.iter() {
        trace_av_err!(
            unsafe { av_dict_set(&mut dict_ptr, key.as_ptr(), value.as_ptr(), 0) },
            "Error while setting dict entry {:?}",
            (key, value)
        )?;
        // todo: deallocate dict if error
    }

    Ok(dict_ptr)
}

fn set_options(class_ptr: *mut AVClass, options: Vec<FfmpegOption>) -> StrResult<()> {
    let class_ptr = class_ptr as _;
    for FfmpegOption(name, value) in options {
        let name_c_string = trace_err!(CString::new(name))?;

        trace_av_err!(
            unsafe {
                match value.clone() {
                    FfmpegOptionValue::String(value) => {
                        let value_c_string = trace_err!(CString::new(value))?;
                        av_opt_set(
                            class_ptr,
                            name_c_string.as_ptr(),
                            value_c_string.as_ptr(),
                            0,
                        )
                    }
                    FfmpegOptionValue::Int(value) => {
                        av_opt_set_int(class_ptr, name_c_string.as_ptr(), value, 0)
                    }
                    FfmpegOptionValue::Double(value) => {
                        av_opt_set_double(class_ptr, name_c_string.as_ptr(), value, 0)
                    }
                    FfmpegOptionValue::Rational { num, den } => av_opt_set_q(
                        class_ptr,
                        name_c_string.as_ptr(),
                        AVRational { num, den },
                        0,
                    ),
                    FfmpegOptionValue::Binary(value) => av_opt_set_bin(
                        class_ptr,
                        name_c_string.as_ptr(),
                        value.as_ptr(),
                        value.len() as _,
                        0,
                    ),
                    FfmpegOptionValue::ImageSize { width, height } => {
                        av_opt_set_image_size(class_ptr, name_c_string.as_ptr(), width, height, 0)
                    }
                    FfmpegOptionValue::VideoRate { num, den } => av_opt_set_video_rate(
                        class_ptr,
                        name_c_string.as_ptr(),
                        AVRational { num, den },
                        0,
                    ),
                    FfmpegOptionValue::ChannelLayout(value) => {
                        av_opt_set_channel_layout(class_ptr, name_c_string.as_ptr(), value, 0)
                    }
                    FfmpegOptionValue::Dictionary(entries) => {
                        let mut dict_ptr = alloc_and_fill_av_dict(entries)?;
                        let res =
                            av_opt_set_dict_val(class_ptr, name_c_string.as_ptr(), dict_ptr, 0);
                        av_dict_free(&mut dict_ptr);
                        res
                    }
                }
            },
            "Error while setting {:?}",
            (name_c_string, value)
        )?;
    }
    Ok(())
}

pub enum FfmpegResultOk {
    Some,
    Again,
    EOF,
}

pub struct FfmpegFrame {
    pub texture: Arc<Texture>,
    av_frame_id: usize,
}

pub struct FfmpegPacket {
    pub frame_id: u64,
    pub data: Vec<u8>,
    pub size: usize,
    av_packet_id: usize,
}

pub struct FfmpegVideoEncoder {
    context_ptr: *mut AVCodecContext,
    frame_ptrs: Vec<*mut AVFrame>,
    packet_ptrs: Vec<*mut AVPacket>,
}

unsafe impl Send for FfmpegVideoEncoder {}
unsafe impl Sync for FfmpegVideoEncoder {}

impl FfmpegVideoEncoder {
    pub fn new(
        resolution: (u32, u32),
        fps: f32,
        encoder_name: &str,
        video_encoder_desc: FfmpegVideoEncoderDesc,
    ) -> StrResult<Self> {
        let codec_ptr = trace_null_ptr!(avcodec_find_encoder_by_name(to_c_str(encoder_name)?))?;

        let context_ptr = trace_null_ptr!(avcodec_alloc_context3(codec_ptr))?;
        let (width, height) = resolution;
        unsafe {
            (*context_ptr).width = width as _;
            (*context_ptr).height = height as _;
            (*context_ptr).time_base = AVRational {
                num: 1,
                den: fps as _,
            };
            (*context_ptr).framerate = AVRational {
                num: fps as _,
                den: 1,
            };
            (*context_ptr).max_b_frames = 0;
            (*context_ptr).pix_fmt = PIXEL_FORMAT;
        }
        //todo: set in settings: bit_rate, gop_size
        set_options(context_ptr as _, video_encoder_desc.context_options)?;
        // todo: set nvenc/amf/etc options in settings: i.e. preset
        set_options(context_ptr as _, video_encoder_desc.priv_data_options)?;

        let mut opts_ptr = alloc_and_fill_av_dict(video_encoder_desc.codec_open_options)?;
        trace_av_err!(unsafe { avcodec_open2(context_ptr, codec_ptr, &mut opts_ptr) })?;
        // todo: read encoder set opts
        unsafe { av_dict_free(&mut opts_ptr) };

        // let packet_ptr = trace_null_ptr!(av_packet_alloc())?;

        // let frame_ptr = trace_null_ptr!(av_frame_alloc())?;
        // unsafe {
        //     // AVPixelFormat is repr(c), so it can be transmuted
        //     (*frame_ptr).format = transmute((*context_ptr).pix_fmt);
        //     (*frame_ptr).width = (*context_ptr).width;
        //     (*frame_ptr).height = (*context_ptr).height;
        // }
        // set_options(frame_ptr as _, video_encoder_desc.frame_options)?;

        // trace_av_err!(av_frame_get_buffer(frame_ptr, number_of_buffers as _))?;

        //https://stackoverflow.com/questions/49862610/opengl-to-ffmpeg-encode
        if let FfmpegVideoEncoderType::CudaNvenc = video_encoder_desc.encoder_type {
            trace_err!(cuda_init())?;
            let cu_device = trace_err!(CudaDevice::get_current())?;
            let cu_device_props = trace_err!(cu_device.get_properties())?;

            let mut device_ref_ptr = null_mut();
            trace_av_err!(unsafe {
                av_hwdevice_ctx_create(
                    &mut device_ref_ptr,
                    AVHWDeviceType::AV_HWDEVICE_TYPE_CUDA,
                    cu_device_props.name.as_ptr(),
                    null_mut(),
                    0,
                )
            })?;
            let device_context_ptr = unsafe { (*device_ref_ptr).data } as *mut AVHWDeviceContext;
            let hw_context_ptr = unsafe { (*device_context_ptr).hwctx } as *mut AVCUDADeviceContext;
            let cu_context = unsafe { (*hw_context_ptr).cuda_ctx };

            let frame_ref_ptr = trace_null_ptr!(av_hwframe_ctx_alloc(device_ref_ptr))?;
            let frame_context_ptr = unsafe { (*frame_ref_ptr).data } as *mut AVHWFramesContext;
            unsafe {
                (*frame_context_ptr).width = width as _;
                (*frame_context_ptr).height = height as _;
                (*frame_context_ptr).sw_format = AV_PIX_FMT_0BGR32;
                (*frame_context_ptr).format = AVPixelFormat::AV_PIX_FMT_CUDA;
                (*frame_context_ptr).device_ref = device_ref_ptr;
                (*frame_context_ptr).device_ctx = device_context_ptr;
            }
            trace_av_err!(unsafe { av_hwframe_ctx_init(frame_ref_ptr) })?;

            let mut old_cu_context = null_mut();
            unsafe { cuCtxPopCurrent_v2(&mut old_cu_context) };
            trace_cu_err!(cuCtxPushCurrent_v2(cu_context))?;

            // todo convert image to buffer, then buffer to cuda buffer

            todo!()
        }

        Ok(FfmpegVideoEncoder {
            context_ptr,
            frame_ptrs: vec![],
            packet_ptrs: vec![],
        })
    }

    pub fn encode(&self) {}
}

impl Drop for FfmpegVideoEncoder {
    fn drop(&mut self) {
        unsafe {
            avcodec_free_context(&mut self.context_ptr);
            for p in &mut self.frame_ptrs {
                av_frame_free(p);
            }
            for p in &mut self.packet_ptrs {
                av_packet_free(p);
            }
        }
    }
}

pub struct FfmpegVideoDecoder {
    context_ptr: *mut AVCodecContext,
    frame_ptrs: Vec<*mut AVFrame>,
    packet_ptrs: Vec<*mut AVPacket>,
}

unsafe impl Send for FfmpegVideoDecoder {}
unsafe impl Sync for FfmpegVideoDecoder {}

impl FfmpegVideoDecoder {
    pub fn new(
        resolution: (u32, u32),
        decoder_name: &str,
        video_decoder_desc: FfmpegVideoDecoderDesc,
    ) -> StrResult<Self> {
        let codec_ptr = trace_null_ptr!(avcodec_find_decoder_by_name(to_c_str(decoder_name)?))?;

        let context_ptr = trace_null_ptr!(avcodec_alloc_context3(codec_ptr))?;
        let (width, height) = resolution;
        unsafe {
            (*context_ptr).width = width as _;
            (*context_ptr).height = height as _;
        }
        set_options(context_ptr as _, video_decoder_desc.context_options)?;
        set_options(context_ptr as _, video_decoder_desc.priv_data_options)?;

        let mut opts_ptr = alloc_and_fill_av_dict(video_decoder_desc.codec_open_options)?;
        trace_av_err!(unsafe { avcodec_open2(context_ptr, codec_ptr, &mut opts_ptr) })?;
        unsafe { av_dict_free(&mut opts_ptr) };

        let frame_ptr = trace_null_ptr!(av_frame_alloc())?;
        unsafe {
            (*frame_ptr).format = transmute((*context_ptr).pix_fmt);
            (*frame_ptr).width = (*context_ptr).width;
            (*frame_ptr).height = (*context_ptr).height;
        }
        set_options(frame_ptr as _, video_decoder_desc.frame_options)?;

        Ok(FfmpegVideoDecoder {
            context_ptr,
            frame_ptrs: vec![],
            packet_ptrs: vec![],
        })
    }

    pub fn create_packets(&mut self, count: usize, size: usize) -> StrResult<Vec<FfmpegPacket>> {
        let mut packets = vec![];
        for _ in 0..count {
            let data = vec![0; size];
            let packet_ptr = trace_null_ptr!(av_packet_alloc())?;
            unsafe { (*packet_ptr).data = data.as_ptr() as _ };
            // `size` field is set before encoding

            packets.push(FfmpegPacket {
                frame_id: 0,
                data,
                size: 0,
                av_packet_id: packet_ptr as _,
            });
            // Save pointer to deallocate packet when FfmpegVideoDecoder drops
            self.packet_ptrs.push(packet_ptr);
        }
        Ok(packets)
    }

    pub fn decode(
        &mut self,
        packet: Option<&FfmpegPacket>,
        frame: &FfmpegFrame,
    ) -> StrResult<FfmpegResultOk> {
        if let Some(packet) = packet {
            let packet_ptr = packet.av_packet_id as *mut AVPacket;
            unsafe { (*packet_ptr).size = packet.size as _ }
            trace_av_err!(unsafe { avcodec_send_packet(self.context_ptr, packet_ptr) })?;
        }

        let res = unsafe { avcodec_receive_frame(self.context_ptr, frame.av_frame_id as _) };
        if res == AVERROR(EAGAIN) {
            Ok(FfmpegResultOk::Again)
        } else if res == AVERROR_EOF {
            Ok(FfmpegResultOk::EOF)
        } else {
            trace_av_err!(res).map(|_| FfmpegResultOk::Some)
        }
    }
}

impl Drop for FfmpegVideoDecoder {
    fn drop(&mut self) {
        unsafe {
            avcodec_free_context(&mut self.context_ptr);
            for p in &mut self.frame_ptrs {
                av_frame_free(p);
            }
            for p in &mut self.packet_ptrs {
                av_packet_free(p);
            }
        }
    }
}
