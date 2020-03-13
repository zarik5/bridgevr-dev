# Strategies

## gfx-hal + ffmpeg

Gfx should be capable of compute pipelines.

Pros:

* One graphics implementation
* Use platform native graphics apis, no d3d-vk interop
* Hardware input support for macos videotoolbox
* less glue for ffmpeg input configuration
* Easier support for future pc hardware decoder

Cons:

* Unsafe graphics
* need to mantain a modded version of gfx-hal with access to internals
* lots of glue between modules (multiplied for each graphics api)
* No ffmpeg hardware input support for combination linux-amd, need a separate implementation with AMF ffi
* No ffmpeg hardware input support for mediacodec
* ffmpeg hard to compile and distribute

Modules needed:

* graphics
* ffmepeg
* amf encoder
* mediacodec

Glue:

* instance and device creation from openvr handles
* pass instance and device to openvr
* image creation from openvr handle
* pass image to openvr

## ash + native codec apis

Pros:

* One graphics implementation
* Less glue for moltenvk-vk

Cons:

* lots of glue for each encoder api, or complicated macros
* Need 3 separate encoder implementations (nvenc, amf, videotoolbox)
* More glue for d3d-vk, potentially expensive interop

Modules needed:

* vulkan
* nvenc
* amf encoder
* videotoolbox
* mediacodec

## ash + ffmpeg

Modules:

* vulkan
* ffmepeg
* amf encoder
* mediacodec

## gfx-hal + native codec apis

Modules:

* graphics
* nvenc
* amf encoder
* videotoolbox
* mediacodec

## Developement plan

* Start with ash, rendering pipelines, ubuntu only
* Develop minimum app (ffmpeg nvenc only, mediacodec, ovr video, no audio, no ffr)
* Add compute pipelines (no substitution)
* Convert to gfx-hal
* Add the rest for ubuntu (ffmpeg amf, ovr input, audio, ffr, latency minimization)
* Pubblish
* Add windows and macos support
