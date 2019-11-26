#pragma once

// fake windows.h header with only what is strictly necessary for nvEncodeAPI.h
// plus `static const GUID` fix for bindgen

#include <guiddef.h>
typedef struct tagRECT
{
    long left;
    long top;
    long right;
    long bottom;
} RECT;

// extern "C"
// {
    extern const GUID NV_ENC_CODEC_H264_GUID;
    extern const GUID NV_ENC_CODEC_HEVC_GUID;
    extern const GUID NV_ENC_CODEC_PROFILE_AUTOSELECT_GUID;
    extern const GUID NV_ENC_H264_PROFILE_BASELINE_GUID;
    extern const GUID NV_ENC_H264_PROFILE_MAIN_GUID;
    extern const GUID NV_ENC_H264_PROFILE_HIGH_GUID;
    extern const GUID NV_ENC_H264_PROFILE_HIGH_444_GUID;
    extern const GUID NV_ENC_H264_PROFILE_STEREO_GUID;
    extern const GUID NV_ENC_H264_PROFILE_SVC_TEMPORAL_SCALABILTY;
    extern const GUID NV_ENC_H264_PROFILE_PROGRESSIVE_HIGH_GUID;
    extern const GUID NV_ENC_H264_PROFILE_CONSTRAINED_HIGH_GUID;
    extern const GUID NV_ENC_HEVC_PROFILE_MAIN_GUID;
    extern const GUID NV_ENC_HEVC_PROFILE_MAIN10_GUID;
    extern const GUID NV_ENC_HEVC_PROFILE_FREXT_GUID;
    extern const GUID NV_ENC_PRESET_DEFAULT_GUID;
    extern const GUID NV_ENC_PRESET_HP_GUID;
    extern const GUID NV_ENC_PRESET_HQ_GUID;
    extern const GUID NV_ENC_PRESET_BD_GUID;
    extern const GUID NV_ENC_PRESET_LOW_LATENCY_DEFAULT_GUID;
    extern const GUID NV_ENC_PRESET_LOW_LATENCY_HQ_GUID;
    extern const GUID NV_ENC_PRESET_LOW_LATENCY_HP_GUID;
    extern const GUID NV_ENC_PRESET_LOSSLESS_DEFAULT_GUID;
    extern const GUID NV_ENC_PRESET_LOSSLESS_HP_GUID;
// }