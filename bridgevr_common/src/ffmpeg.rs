#![allow(clippy::cast_ptr_alignment)]

use crate::{data::*, rendering::*, *};
use cuda::{driver::*, ffi::cuda::*, runtime::*};
use ffmpeg_sys::*;
use std::{
    ffi::{CStr, CString},
    mem,
    os::raw::*,
    ptr::*,
    sync::Arc,
};

//https://ffmpeg.org/doxygen/trunk/hwcontext_8h.html

//https://www.phoronix.com/scan.php?page=news_item&px=FFmpeg-AMD-AMF-Vulkan

//https://stackoverflow.com/questions/50693934/different-h264-encoders-in-ffmpeg

//https://stackoverflow.com/questions/49862610/opengl-to-ffmpeg-encode
// convert both vulkan and directx texures to cuda array and then copy to ffmpeg cuda array

const TRACE_CONTEXT: &str = "FFmpeg";

// const SOFTWARE_FORMAT: AVPixelFormat = AVPixelFormat::AV_PIX_FMT_BGR24;
// // or AV_PIX_FMT_BGR0 or AV_PIX_FMT_RGB24 for libx264rgb, AV_PIX_FMT_RGB32

// const CUDA_FORMAT: AVPixelFormat = AV_PIX_FMT_0BGR32; //AV_PIX_FMT_RGB32;

const SW_FORMAT: AVPixelFormat = AVPixelFormat::AV_PIX_FMT_NV12;
const FRAME_BUFFER_SIZE_ALIGNMENT: i32 = 32;

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
        )
        .map_err(|res| unsafe {
            av_dict_free(&mut dict_ptr);
            res
        })?;
    }

    Ok(dict_ptr)
}

fn set_options(class_ptr: *mut AVClass, options: Vec<FfmpegOption>) -> StrResult {
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
    SomeOutput,
    NoOutput,
}

pub struct FfmpegPacket {
    pub frame_id: usize,
    pub data: Vec<u8>,
    pub data_offset: usize,
    pub size: usize,
    av_packet: AVPacket,
}

struct FfmpegVideoCoder {
    context_ptr: *mut AVCodecContext,
    hw_device_ref_ptr: Option<*mut AVBufferRef>,
    frame_ptr: *mut AVFrame,
}

unsafe impl Send for FfmpegVideoCoder {}
unsafe impl Sync for FfmpegVideoCoder {}

impl FfmpegVideoCoder {
    fn create_packets(
        &mut self,
        count: usize,
        max_size: usize,
        data_offset: usize,
    ) -> Vec<FfmpegPacket> {
        let mut packets = vec![];
        for _ in 0..count {
            let data = vec![0; max_size];
            let mut av_packet = unsafe { mem::zeroed() };
            unsafe { av_init_packet(&mut av_packet) };

            packets.push(FfmpegPacket {
                frame_id: 0,
                data,
                data_offset,
                size: 0,
                av_packet,
            });
        }
        packets
    }
}

impl Drop for FfmpegVideoCoder {
    fn drop(&mut self) {
        unsafe {
            av_frame_free(&mut self.frame_ptr);
            if let Some(mut hw_device_ref_ptr) = self.hw_device_ref_ptr {
                av_buffer_unref(&mut hw_device_ref_ptr);
            }
            avcodec_free_context(&mut self.context_ptr);
        }
    }
}

pub struct FfmpegVideoEncoder {
    encoder_type: FfmpegVideoEncoderType,
    video_coder: FfmpegVideoCoder,
}

impl FfmpegVideoEncoder {
    pub fn new(
        resolution: (u32, u32),
        fps: f32,
        video_encoder_desc: FfmpegVideoEncoderDesc,
    ) -> StrResult<Self> {
        let encoder_type = video_encoder_desc.encoder_type;

        let context_format;
        let mut maybe_hw_device_type = None;
        let maybe_hw_name_c_str = None;
        match encoder_type {
            #[cfg(target_os = "linux")]
            FfmpegVideoEncoderType::CUDA => {
                context_format = AVPixelFormat::AV_PIX_FMT_CUDA;
                maybe_hw_device_type = Some(AVHWDeviceType::AV_HWDEVICE_TYPE_CUDA);

                trace_err!(cuda_init())?;
                let cu_device = trace_err!(CudaDevice::get_current())?;
                let cu_device_props = trace_err!(cu_device.get_properties())?;
                maybe_hw_name_c_str =
                    Some(unsafe { CStr::from_ptr(&cu_device_props.name as _).to_owned() });
            }
            #[cfg(windows)]
            FfmpegVideoEncoderType::D3D11VA => {
                context_format = AVPixelFormat::AV_PIX_FMT_D3D11;
                maybe_hw_device_type = Some(AVHWDeviceType::AV_HWDEVICE_TYPE_D3D11VA);
            }
            #[cfg(target_os = "macos")]
            FfmpegVideoEncoderType::D3D11VA => {
                context_format = AVPixelFormat::AV_PIX_FMT_VIDEOTOOLBOX;
                maybe_hw_device_type = Some(AVHWDeviceType::AV_HWDEVICE_TYPE_VIDEOTOOLBOX);
            }
            FfmpegVideoEncoderType::Software => context_format = SW_FORMAT,
        }

        let mut maybe_hw_device_ref_ptr = None;
        if let Some(hw_device_type) = maybe_hw_device_type {
            let hw_name_ptr = maybe_hw_name_c_str.map_or(null(), |c_str: CString| c_str.as_ptr());

            let mut hw_device_ref_ptr = null_mut();
            trace_av_err!(unsafe {
                av_hwdevice_ctx_create(
                    &mut hw_device_ref_ptr,
                    hw_device_type,
                    hw_name_ptr,
                    null_mut(),
                    0,
                )
            })?;
            maybe_hw_device_ref_ptr = Some(hw_device_ref_ptr);
        }

        let codec_ptr = trace_null_ptr!(avcodec_find_encoder_by_name(to_c_str(
            &video_encoder_desc.encoder_name
        )?))?;

        let context_ptr = trace_null_ptr!(avcodec_alloc_context3(codec_ptr))?;
        let (width, height) = resolution;
        unsafe {
            (*context_ptr).width = width as _;
            (*context_ptr).height = height as _;
            (*context_ptr).pix_fmt = context_format;
            (*context_ptr).sw_pix_fmt = SW_FORMAT;
            (*context_ptr).time_base = AVRational {
                num: 1,
                den: fps as _,
            };
            (*context_ptr).framerate = AVRational {
                num: fps as _,
                den: 1,
            };
        }
        //todo: set in settings: bit_rate, gop_size, max_b_frames = 0
        set_options(context_ptr as _, video_encoder_desc.context_options)?;
        // todo: set nvenc/amf/etc options in settings: i.e. preset
        set_options(context_ptr as _, video_encoder_desc.priv_data_options)?;

        let mut maybe_hw_frames_ref_ptr = None;
        if let Some(hw_device_ref_ptr) = maybe_hw_device_ref_ptr {
            let mut hw_frames_ref_ptr = trace_null_ptr!(av_hwframe_ctx_alloc(hw_device_ref_ptr))?;
            let frames_ctx_ptr = unsafe { (*hw_frames_ref_ptr).data } as *mut AVHWFramesContext;
            unsafe {
                (*frames_ctx_ptr).width = width as _;
                (*frames_ctx_ptr).height = height as _;
                (*frames_ctx_ptr).format = context_format;
                (*frames_ctx_ptr).sw_format = SW_FORMAT;
                (*frames_ctx_ptr).device_ref = hw_device_ref_ptr;
                (*frames_ctx_ptr).device_ctx = (*hw_device_ref_ptr).data as _;
            }
            set_options(
                frames_ctx_ptr as _,
                video_encoder_desc.hw_frames_context_options,
            )?;
            // todo: set initial_pool_size (= 20) in settings

            trace_av_err!(unsafe { av_hwframe_ctx_init(hw_frames_ref_ptr) }).map_err(
                |res| unsafe {
                    av_buffer_unref(&mut hw_frames_ref_ptr);
                    res
                },
            )?;

            // let new_hw_frames_ref_ptr = av_buffer_ref(hw_frames_ref_ptr);
            unsafe {
                (*context_ptr).hw_device_ctx = hw_device_ref_ptr;
                (*context_ptr).hw_frames_ctx = hw_frames_ref_ptr;
                (*context_ptr).sw_pix_fmt = SW_FORMAT;
            };
            // av_buffer_unref(&mut hw_frames_ref_ptr);
            // trace_null_ptr!(new_hw_frames_ref_ptr)?;

            maybe_hw_frames_ref_ptr = Some(hw_frames_ref_ptr)
        }

        let mut opts_ptr = alloc_and_fill_av_dict(video_encoder_desc.codec_open_options)?;
        trace_av_err!(unsafe { avcodec_open2(context_ptr, codec_ptr, &mut opts_ptr) })?;
        // todo: read encoder set opts
        unsafe { av_dict_free(&mut opts_ptr) };

        let frame_ptr = trace_null_ptr!(av_frame_alloc())?;
        set_options(frame_ptr as _, video_encoder_desc.frame_options)?;
        if let Some(hw_frames_ref_ptr) = maybe_hw_frames_ref_ptr {
            trace_av_err!(unsafe { av_hwframe_get_buffer(hw_frames_ref_ptr, frame_ptr, 0) })?;
            trace_null_ptr!((*frame_ptr).hw_frames_ctx)?;
        } else {
            unsafe {
                (*frame_ptr).width = width as _;
                (*frame_ptr).height = height as _;
                (*frame_ptr).format = mem::transmute(SW_FORMAT);
            }
            trace_av_err!(unsafe { av_frame_get_buffer(frame_ptr, FRAME_BUFFER_SIZE_ALIGNMENT) })?;
        }

        // if encoder_type != FfmpegVideoEncoderType::Software {

        //         let mut old_cu_context = null_mut();
        //         unsafe { cuCtxPopCurrent_v2(&mut old_cu_context) };
        //         trace_cu_err!(cuCtxPushCurrent_v2(cu_context))?;

        //         // todo convert image to buffer, then buffer to cuda buffer

        //         todo!()
        //     }
        // }

        Ok(FfmpegVideoEncoder {
            encoder_type,
            video_coder: FfmpegVideoCoder {
                context_ptr,
                hw_device_ref_ptr: maybe_hw_device_ref_ptr,
                frame_ptr,
            },
        })
    }

    pub fn create_packets(
        &mut self,
        count: usize,
        max_size: usize,
        data_offset: usize,
    ) -> Vec<FfmpegPacket> {
        self.video_coder
            .create_packets(count, max_size, data_offset)
    }

    pub fn submit_frame(
        &self,
        frame_id: usize,
        texture_callback: impl FnOnce(&Arc<Texture>),
    ) -> StrResult {
        let frame_ptr = self.video_coder.frame_ptr;
        unsafe { (*frame_ptr).pts = frame_id as _ };

        match self.encoder_type {
            #[cfg(target_os = "linux")]
            FfmpegVideoEncoderType::CUDA => todo!(),
            #[cfg(windows)]
            FfmpegVideoEncoderType::D3D11VA => todo!(),
            #[cfg(target_os = "macos")]
            FfmpegVideoEncoderType::VideoToolbox => todo!(),
            FfmpegVideoEncoderType::Software => {
                trace_av_err!(unsafe { av_frame_make_writable(frame_ptr) })?;

                // let texture = Texture::

                // let frame_data = texture.read()?;

                // todo: check color channel order
                // for i in 0..frame_data.len() / 4 {
                //     for c in 0..3 {
                //         unsafe { *(*frame_ptr).data[c].add(i) = frame_data[i * 4 + c] }
                //     }
                // }
            }
        }

        trace_av_err!(unsafe { avcodec_send_frame(self.video_coder.context_ptr, frame_ptr) })
    }

    pub fn receive_packet(&mut self, packet: &mut FfmpegPacket) -> StrResult<FfmpegResultOk> {
        packet.av_packet.data = null_mut();
        packet.av_packet.size = 0;
        packet.av_packet.stream_index = 0;

        let res =
            unsafe { avcodec_receive_packet(self.video_coder.context_ptr, &mut packet.av_packet) };
        if res == AVERROR_EOF || res == AVERROR(EAGAIN) {
            Ok(FfmpegResultOk::NoOutput)
        } else {
            trace_av_err!(res)?;

            // todo: try to avoid copy?
            // I could send the packet where I overwrite the first 8 bytes for a packet
            // index, then I send another packet with the same index, the header and
            // the overwritten 8 bytes.
            packet.size = packet.av_packet.size as _;
            unsafe {
                copy_nonoverlapping(
                    packet.av_packet.data,
                    &mut packet.data[packet.data_offset],
                    packet.size,
                )
            };

            packet.frame_id = packet.av_packet.pts as _;

            unsafe { av_packet_unref(&mut packet.av_packet) }
            Ok(FfmpegResultOk::SomeOutput)
        }
    }
}

pub struct FfmpegVideoDecoder {
    decoder_type: FfmpegVideoDecoderType,
    video_coder: FfmpegVideoCoder,
    graphics_al: Arc<GraphicsAbstractionLayer>,
}

impl FfmpegVideoDecoder {
    pub fn new(
        graphics_al: Arc<GraphicsAbstractionLayer>,
        resolution: (u32, u32),
        video_decoder_desc: FfmpegVideoDecoderDesc,
    ) -> StrResult<Self> {
        let decoder_type = video_decoder_desc.decoder_type;

        let hw_format;
        let hw_device_type;
        match decoder_type {
            FfmpegVideoDecoderType::MediaCodec => {
                hw_format = AVPixelFormat::AV_PIX_FMT_MEDIACODEC;
                hw_device_type = AVHWDeviceType::AV_HWDEVICE_TYPE_MEDIACODEC;
            }
            FfmpegVideoDecoderType::D3D11VA => {
                hw_format = AVPixelFormat::AV_PIX_FMT_D3D11;
                hw_device_type = AVHWDeviceType::AV_HWDEVICE_TYPE_D3D11VA;
            }
        };

        let codec_ptr = trace_null_ptr!(avcodec_find_decoder_by_name(to_c_str(
            &video_decoder_desc.decoder_name
        )?))?;

        let context_ptr = trace_null_ptr!(avcodec_alloc_context3(codec_ptr))?;
        let (width, height) = resolution;
        unsafe {
            (*context_ptr).width = width as _;
            (*context_ptr).height = height as _;
            (*context_ptr).pix_fmt = hw_format;
            (*context_ptr).sw_pix_fmt = SW_FORMAT;
        }
        set_options(context_ptr as _, video_decoder_desc.context_options)?;
        set_options(context_ptr as _, video_decoder_desc.priv_data_options)?;

        let mut hw_device_ref_ptr = null_mut();
        trace_av_err!(unsafe {
            av_hwdevice_ctx_create(
                &mut hw_device_ref_ptr,
                hw_device_type,
                null_mut(),
                null_mut(),
                0,
            )
        })?;

        let mut hw_frames_ref_ptr = trace_null_ptr!(av_hwframe_ctx_alloc(hw_device_ref_ptr))?;
        let frames_ctx_ptr = unsafe { (*hw_frames_ref_ptr).data } as *mut AVHWFramesContext;
        unsafe {
            (*frames_ctx_ptr).width = width as _;
            (*frames_ctx_ptr).height = height as _;
            (*frames_ctx_ptr).format = hw_format;
            (*frames_ctx_ptr).sw_format = SW_FORMAT;
            (*frames_ctx_ptr).device_ref = hw_device_ref_ptr;
            (*frames_ctx_ptr).device_ctx = (*hw_device_ref_ptr).data as _;
        }
        set_options(
            frames_ctx_ptr as _,
            video_decoder_desc.hw_frames_context_options,
        )?;
        // todo: set initial_pool_size (= 20) in settings

        trace_av_err!(unsafe { av_hwframe_ctx_init(hw_frames_ref_ptr) }).map_err(|res| unsafe {
            av_buffer_unref(&mut hw_frames_ref_ptr);
            res
        })?;

        unsafe {
            (*context_ptr).hw_device_ctx = hw_device_ref_ptr;
            (*context_ptr).hw_frames_ctx = hw_frames_ref_ptr;
            (*context_ptr).sw_pix_fmt = SW_FORMAT;
        };

        let mut opts_ptr = alloc_and_fill_av_dict(video_decoder_desc.codec_open_options)?;
        trace_av_err!(unsafe { avcodec_open2(context_ptr, codec_ptr, &mut opts_ptr) })?;
        unsafe { av_dict_free(&mut opts_ptr) };

        let frame_ptr = trace_null_ptr!(av_frame_alloc())?;
        set_options(frame_ptr as _, video_decoder_desc.frame_options)?;
        trace_av_err!(unsafe { av_hwframe_get_buffer(hw_frames_ref_ptr, frame_ptr, 0) })?;
        trace_null_ptr!((*frame_ptr).hw_frames_ctx)?;

        Ok(FfmpegVideoDecoder {
            decoder_type,
            video_coder: FfmpegVideoCoder {
                context_ptr,
                hw_device_ref_ptr: Some(hw_device_ref_ptr),
                frame_ptr,
            },
            graphics_al,
        })
    }

    pub fn create_packets(
        &mut self,
        count: usize,
        max_size: usize,
        data_offset: usize,
    ) -> Vec<FfmpegPacket> {
        self.video_coder
            .create_packets(count, max_size, data_offset)
    }

    // pub fn create_frames(&mut self, textures: Vec<Arc<Texture>>) -> StrResult<Vec<FfmpegFrame>> {
    //     self.video_coder.create_frames(textures)
    // }

    pub fn decode(
        &mut self,
        packet: &mut FfmpegPacket,
        texture_callback: impl FnOnce(&Texture, u64),
    ) -> StrResult<FfmpegResultOk> {
        packet.av_packet.data = &mut packet.data[packet.data_offset];
        packet.av_packet.size = packet.size as _;
        packet.av_packet.pts = packet.frame_id as _;
        trace_av_err!(unsafe {
            avcodec_send_packet(self.video_coder.context_ptr, &packet.av_packet)
        })?;

        let frame_ptr = self.video_coder.frame_ptr;
        let mut filled = false;
        loop {
            let res = unsafe { avcodec_receive_frame(self.video_coder.context_ptr, frame_ptr) };
            if res == AVERROR_EOF || res == AVERROR(EAGAIN) {
                if filled {
                    let texture_ptr = unsafe { (*frame_ptr).data[0] };
                    let frame_id = unsafe { (*frame_ptr).pts } as _;

                    match self.decoder_type {
                        FfmpegVideoDecoderType::MediaCodec => todo!(),
                        FfmpegVideoDecoderType::D3D11VA => {
                            let texture =
                                Texture::from_handle(texture_ptr as _, self.graphics_al.clone())?;
                            texture_callback(&texture, frame_id);
                        }
                    }

                    break Ok(FfmpegResultOk::SomeOutput);
                } else {
                    break Ok(FfmpegResultOk::NoOutput);
                }
            } else {
                trace_av_err!(res)?;
                filled = true;
            }
        }
    }
}
