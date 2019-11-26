# How BridgeVR works internally

## Language and libraries

BridgeVR is written in Rust, a fast and secure language that catches most memory related bugs at compile time by prohibiting nullability and enforcing RAII. Rust was also choosen for its powerful enums (tagged unions) that allow storing without redundancy diverse types of data and writing code where the predominant type of branching is exaustive `match` (`switch` in C/C++/Java).  

The most important libraries used by BridgeVR are:

* OpenVR driver for the VR server (through a custom wrapper). Contrary to ALVR which uses the DirectModeDriver interface, BridgeVR uses the VirtualDisplay interface.
* OpenXR for the VR client. This allows BridgeVR to potentially support an increasing number of VR headsets.
* Servo for settings and packets de/serialization.
* NVENC (through FFI bindings, Windows 10 only) and GStreamer for media encoding/decoding (AFAIK this is the only actively mantained audio/video Rust library that is both cross platform and can support hardware acceleration). This allows BridgeVR to support a wide range of pc and client hardware.
* CPAL for audio recording/playback.

## Client-server communication

The client and the server communicate via Wi-Fi mainly through UDP for minimal latency.  
The handshake happens following the procedure shown here:  

```handshake
      Server      Client
        |           |
       Join        Join
    multicast   multicast
        |           |
        |     UDP multicast
        |         config
        |        /  |
   UDP listener <   |
     receive        |  -------> Client found
        |           |
   TCP connect      |
        |   \       |
        |    > TCP listener
        |         accept
     TCP send       |
      config        |
        |   \       |
        |    > TCP receive  --> Client/server connected
        |           |
```

After the handshake, through UDP, the server sends to the client the video audio data and the client sends head and controllers position, controllers input and other metadata (more on this later). The TCP channel created during handshake is kept open and used to send the shutdown signal from both server and client.  

## Latency

### Problem statement

The motion-to-photon pipeline is as follows: the client polls and sends the head and controller poses to the server, which uses them to render, encode and transmit back the frames. The client receives, decodes and shows the frames.  
The server and the client have one render loop each that run at the same frequency. The server one can be offset by returning `time_since_last_offset` when asked by SteamVR runtime. The client one could but shouldn't be offset.  
The goal is to synchronize the two render loops and to minimize the motion-to-photon latency.

### Algorithm

I define a `EventTiming` structure that records the time when it is notified. Through statistical analysis the structure outputs a duration. This duration should be used to re-time the events that called notify.

Let's say we want to display the n-th frame:
• The client sends the pose data `total_latency` seconds before the supposed n-th vsync, along with the frame index (n).
• The server receives the pose data, forwards it immediately to SteamVR runtime and notifies a server EventTiming instance with the frame index.
• When SteamVR calls `Present`, the server recognizes the frame index using the provided pose matrix and notifies the server EventTiming instance with the frame index which returns the `vsync offset` which is added to `time since last vsync` that will be returned to SteamVR when asked.
• In the meanwhile the frame is encoded and transmitted to the client along with head pose and frame index (n). When received, the client start to decode it immediately.
• When decode is finished a client EventTiming instance is notified.
• When it's time to submit the frame for display, the client EventTiming instance is notified and it returns `latency_offset` which is used to offset the (the variable used at the beginning).



To ensure the best VR experience, BridgeVR uses a simple statistical analysis and feedback to adjust in real-time the latency and the used bandwidth for the video/audio stream.

The following pseudo code explains how the rendering and timing code works:  

```timing
Client                              Server

additional_vsync_offset = 0         frame_idx = 0
composition_latency_queue = empty   vsync_offset = 0

LOOP {                              LOOP {

Query next_client_vsync,             Wait;
    head_pose and
    controllers_pose;

(next_client_vsync, head_pose,
    controller_poses,
    additional_vsync_offset) -----> Submit head_pose, controllers_pose;
                                    callback get_time_since_last_vsync() {
Wait;                                 // Virtual vsync
                                      vsync_offset +=
                                          additional_vsync_offset;
                                      return next_client_vsync - 1 / FPS +
                                          vsync_offset;
                                    }

                                    Receive frame from SteamVR;
                                    frame_idx++;
                                    encode frame {
                                      YIELD (frame_idx, sub_frame_idx,
     ,-----------------------------       sub_frame_data)
    /                                };
{  <
  Submit sub_frame_data to decoder;
}

Decode, composite and queue frame;
compostiton_time = now;
Record (next_client_vsync -
    compostiton_time) in
    composition_latency_queue;

// Statistical analysis
mean = mean(
    composition_latency_queue);
IF AUTOMATIC_LATENCY {
  stddev = standard_deviation(
      composition_latency_queue);
  accepted_frame_miss_prob =
      EXPECTED_MISSED_FRAMES_PER_HOUR /
      (60 * 60 * FPS);
  // Here I assume the latency
  // values are distributed as
  // a gaussian variable [*]
  target_composition_latency =
      stddev * inverse_Q(
          accepted_frame_miss_prob);
} ELSE {
  target_latency = TARGET_LATENCY;
}
additional_vsync_offset =
    (target_latency - mean) /
    LATENCY_HISTORY_SIZE;

Wait for vsync;

Try dequeue frame for this vsync;
IF frame missing {
  Submit last composited frame;
} ELSE {
  Submit current composited frame;
}

}                                   }
```

[*] [Q funtion](https://en.wikipedia.org/wiki/Q-function)

Pratically BridgeVR uses multiple threads to parallelize the previous algorithm.

Here with composition latency I'm referring to the idle time between the moment the frame is ready and the moment the frame is submitted for display.  
If manual latency is choosen in settings, the latency is regulated using the mean of the past values. If automatic latency is set, also the standard deviation is used, which is an higher order statistic, so it requires an higher value for `latency_history_size` to avoid jitter.

Note that neither during handshake nor during streaming was there a clock syncing between server and client. This sync-less architechture is resilient to packet loss and clock drift.

## Foveated Rendering and sliced encoding

BridgeVR implements Oculus' AADT (Axis-Aligned Distorted Transfer) foveated rendering technique where the edges of the frames are squished to reduce the amount of data transmitted and then re-expanded before displaying the frames.  
Sliced encoding is also implemented where each frame is decomposed into smaller rectangles and then encoded, transmitted and decoded independent from each other; finally the slices are merged into the original frame. The size and arrangement of the slices is calculated so to minimize the wasted bandwidth by new objects entering the slice.
