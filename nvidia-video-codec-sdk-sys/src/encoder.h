#pragma once

#include "nvEncodeAPI.h"

// reimport version constants because bindgen ignores them
#define CONST_VERSION(struct) const uint32_t struct##_VERSION = struct##_VER

CONST_VERSION(NV_ENC_CAPS_PARAM);
CONST_VERSION(NV_ENC_CREATE_INPUT_BUFFER);
CONST_VERSION(NV_ENC_CREATE_BITSTREAM_BUFFER);
CONST_VERSION(NV_ENC_CREATE_MV_BUFFER);
CONST_VERSION(NV_ENC_RC_PARAMS);
CONST_VERSION(NV_ENC_CONFIG);
CONST_VERSION(NV_ENC_INITIALIZE_PARAMS);
CONST_VERSION(NV_ENC_RECONFIGURE_PARAMS);
CONST_VERSION(NV_ENC_PRESET_CONFIG);
CONST_VERSION(NV_ENC_PIC_PARAMS);
CONST_VERSION(NV_ENC_MEONLY_PARAMS);
CONST_VERSION(NV_ENC_LOCK_BITSTREAM);
CONST_VERSION(NV_ENC_LOCK_INPUT_BUFFER);
CONST_VERSION(NV_ENC_MAP_INPUT_RESOURCE);
CONST_VERSION(NV_ENC_REGISTER_RESOURCE);
CONST_VERSION(NV_ENC_STAT);
CONST_VERSION(NV_ENC_SEQUENCE_PARAM_PAYLOAD);
CONST_VERSION(NV_ENC_EVENT_PARAMS);
CONST_VERSION(NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS);
CONST_VERSION(NV_ENCODE_API_FUNCTION_LIST);