# To be Implemented

HWAccel APIs with hardware i/o:

* VDPAU (linux): Nvidia (decoder), AMD (decoder)
* VAAPI (linux): AMD (encoder + decoder), Intel (encoder), Nvidia (decoder)
* DXVA2 (win): Nvidia + AMD + Intel (decoder)
* D3D11VA(win): Nvidia + AMD + Intel (decoder)
* NVENC (win + linux): Nvidia (encoder)
* NVDEC (win + linux): Nvidia (decoder)
* libmfx(win + linux): Intel (encoder + decoder)
* VCE(AMF)(win + linux): AMD (encoder)

Selected encoders:

* Win10/Nvidia: h264_nvenc, AV_HWDEVICE_TYPE_CUDA
* Win7/Nvidia: h264_nvenc, AV_HWDEVICE_TYPE_CUDA
* Linux/Nvidia: h264_nvenc, AV_HWDEVICE_TYPE_CUDA
* Win10/AMD: h264_amf, AV_HWDEVICE_TYPE_ ?
* Win7/AMD: h264_amf, AV_HWDEVICE_TYPE_ ?
* Linux/AMD: h264_vaapi/h264_amf, AV_HWDEVICE_TYPE_VAAPI
* macOS: h264_videotoolbox, AV_HWDEVICE_TYPE_VIDEOTOOLBOX

Selected decoders:

* Win: D3D11VA
* Android: MediaCodec
