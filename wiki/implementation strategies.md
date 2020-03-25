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
* lots of glue between modules (multiplied for each graphics api and ffmpeg hwaccel device)
* No ffmpeg hardware input support for combination linux-amd, need a separate implementation with AMF ffi
* ffmpeg hard to compile and distribute

Modules needed:

* graphics
* ffmepeg
* amf encoder

## ash + native codec apis

Pros:

* One graphics implementation
* Less glue for moltenvk-vk

Cons:

* Even more unsafe graphics
* lots of input configuration glue for each encoder api, or complicated macros
* Need 3 separate encoder implementations (nvenc, amf, videotoolbox) + medicodec decoder
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

## gfx-hal + native codec apis

Modules:

* graphics
* nvenc
* amf encoder
* videotoolbox
* mediacodec

## Developement plan

* Start with ash, compute pipelines, ubuntu only
* Develop minimum app (ffmpeg nvenc only, mediacodec, ovr video, no audio, no ffr)
* Convert to gfx-hal
* Add the rest for ubuntu (ffmpeg amf, ovr input, audio, ffr, latency minimization)
* Publish
* Add windows and macos support
