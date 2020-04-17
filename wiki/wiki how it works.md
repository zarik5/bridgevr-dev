# How BridgeVR works

## Programming language and libraries

BridgeVR is written in Rust, a fast and safe language that catches most memory related bugs at compile time by prohibiting nullability and enforcing RAII. Rust also excels for its powerful enums (tagged unions) that allow storing without redundancy diverse types of data and help limiting the number of invalid or unexpected states the programs can have.

The most important libraries used by BridgeVR are:

* [Serde](https://github.com/serde-rs) for settings and packets de/serialization.
* [Laminar](https://github.com/amethyst/laminar) as the network protocol.
* [gfx-hal](https://github.com/gfx-rs/gfx) as a graphics abstraction layer based on Vulkan.
* [FFmpeg](https://ffmpeg.org/) for video encoder and decoder.
* [CPAL](https://github.com/RustAudio/cpal) and [Oboe](https://github.com/google/oboe) for audio recording/playback.
* [android-glue](https://github.com/rust-windowing/android-rs-glue) for writing all Rust android app.
* [OpenVR](https://github.com/ValveSoftware/openvr) driver as the VR server (through a custom wrapper and FFI bindings). BridgeVR implements both DirectModeDriver interface (the one used by ALVR) and VirtualDisplay interface.
* [OpenXR](https://www.khronos.org/openxr/) as an alternative VR client (limited support). This allows BridgeVR to potentially support an increasing number of VR headsets.
* [Flutter](https://github.com/flutter-rs/flutter-rs) for the GUI.

## Client-server communication

The client and the server communicate via Wi-Fi through the semi-reliable Laminar protocol.  
The handshake happens following the procedure shown here:  

```
     Server      Client
        |           |
      Join        Join
    multicast   multicast
        |           |
        |     UDP multicast
        |        config
        |       /   |
  UDP listener <    |
     receive        |  -----> Client found
        |           |
    Reliable        |
   send config      |
        |     \     |
        |      > Receive  --> Client/server connected
        |           |
```

After the handshake, through UDP, the server sends to the client the video audio data and the client sends head and controllers position, controllers input and other metadata (more on this later). The TCP channel created during handshake is kept open and used to send the shutdown signal from either server or client.  

## Latency, judder and timing

### Latency minimization: Problem statement

We want to minimize the motion-to-photon latency and also the perceived judder. The motion-to-photon latency is the time between the a pose measurement and the display of the rendered frame using that pose. This latency is caused by the execution of a pipeline that takes the pose as input and gives the frame as output.

### Naive approach

A naive approach for minimizing the motion-to-photon latency would be to make an initial guess of the latency, execute the pipeline and if it finished early, run the next iteration a bit later. If it took too long, start the next pipeline iteration a bit earlier. The biggest flaw of this approach is that due to the unpredictable duration of the execution of the pipeline (e.g. due to interference in the network and the encoding time that depends on the amount of information in a frame), half of the time the pipeline would be late and the frame would be displayed ~14ms later (if fps=72). This creates perceived judder.  
A better approach would be to define a time interval where if the pipeline finished outside of it, the next pipeline iteration is anticipated or delayed.

### BridgeVR approach

The approach explained above is workable but there are still a few problems: how big should the time interval be? Also we want to detect what is the absolute minimum latency that still avoids missing the frame submission deadline. But it doesn't exist, at least if we wanted to be 100% sure the pipeline would not be late.  
What we can do is define a statistical model where we give it a probability that the pipeline is not late and it gives back the optimal latency of the system. More concretely this model records the time durations between the completion of the execution of the pipeline and the time of the frame submission, calculates the average and the standard deviation and returns a time offset to be applied to the start of the next pipeline iteration (actually BridgeVR does not work exactly like this due to how the Oculus mobile sdk and OpenVR work). The core of the algorithm can be found in the module `event_timing`.  
Since the OpenVR interface does not allow drivers to provide the pose data on demand and instead does its pose extrapolation, BridgeVR cannot correlate the poses it submitted with the pose (returned by OpenVR) associated with the rendered frame to estimate the game latency. So the pipeline in the the algorithm above should be interpreted only as `[server compositing + encoding + network transmission + decoding + client compositing]` (the pose transmission and game latency are excluded).  
Note: OpenXR already expects the runtime to make these fancy predictions: [link](http://blog.xyzw.us/2020/01/on-openxr-frame-timing.html).  

### Judder minimization and timing

This leaves two more problems: calculate the total latency (for pose prediction, that affects the perceived "lag") and further minimize judder. The judder is minimized by timing the polling and submission of the pose using a model similar to the one described above. The latency for pose prediction can be calculated using a model that uses the average of the difference between the guessed pose with two more poses polled just before frame submission.  

### Audio

Regarding audio (game sound and microphone) right now an analog approach is not possible because the library that BridgeVR uses does not allow to choose neither polling not submission times of the audio samples nor buffer size. When there are no samples to submit, BridgeVR just waits, or when the latency is too high, it drops a few samples.

## Foveated Rendering and sliced encoding

BridgeVR implements Oculus' AADT (Axis-Aligned Distorted Transfer) foveated rendering technique where the edges of the frames are squished to reduce the amount of data transmitted and then re-expanded before displaying the frames.  
Sliced encoding is also implemented where each frame is decomposed into smaller rectangles and then encoded, transmitted and decoded independently from each other; finally the slices are merged into the original frame. The size and arrangement of the slices is calculated so to minimize the wasted bandwidth by new objects entering the slice.
