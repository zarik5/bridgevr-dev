# To be Implemented

## HWAccel APIs with hardware i/o

* VDPAU (linux): Nvidia (decoder), AMD (decoder)
* VAAPI (linux): AMD (encoder + decoder), Intel (encoder), Nvidia (decoder)
* DXVA2 (win): Nvidia + AMD + Intel (decoder)
* D3D11VA(win): Nvidia + AMD + Intel (decoder)
* NVENC (win + linux): Nvidia (encoder)
* NVDEC (win + linux): Nvidia (decoder)
* libmfx(win + linux): Intel (encoder + decoder)
* VCE(AMF)(win + linux): AMD (encoder)

## Supported formats

### Encoder

* libx264: AV_PIX_FMT_YUV420P, AV_PIX_FMT_BGR0, AV_PIX_FMT_RGB24, AV_PIX_FMT_NV12, ...
* NVENC: AV_PIX_FMT_YUV420P, AV_PIX_FMT_NV12, AV_PIX_FMT_0RGB32, AV_PIX_FMT_0BGR32, AV_PIX_FMT_CUDA, AV_PIX_FMT_D3D11, ...
* AMF: AV_PIX_FMT_NV12, AV_PIX_FMT_YUV420P, AV_PIX_FMT_D3D11, AV_PIX_FMT_BGR0, AV_PIX_FMT_RGB0, AV_PIX_FMT_YUYV422, ..., vulkan soon?
* VideoToolbox: AV_PIX_FMT_NV12, AV_PIX_FMT_P010, AV_PIX_FMT_VIDEOTOOLBOX

Encoder common sw: AV_PIX_FMT_NV12 (or AV_PIX_FMT_YUV420P if no macos)
Encoder hw windows: AV_PIX_FMT_D3D11
Encoder hw linux: AV_PIX_FMT_CUDA (NVENC)
Encoder hw macos: AV_PIX_FMT_VIDEOTOOLBOX,

### Decoder

* MediaCodec: AV_PIX_FMT_YUV420P, AV_PIX_FMT_NV12,
* D3D11VA: AV_PIX_FMT_NV12, AV_PIX_FMT_P010, AV_PIX_FMT_YUV420P

Decoder common sw: AV_PIX_FMT_YUV420P, AV_PIX_FMT_NV12

## Supported backends

* AMF: AV_HWDEVICE_TYPE_D3D11VA, AV_HWDEVICE_TYPE_DXVA2, vulkan soon?
* NVENC: AV_HWDEVICE_TYPE_CUDA, AV_HWDEVICE_TYPE_D3D11VA
* VideoToolbox: AV_HWDEVICE_TYPE_VIDEOTOOLBOX
