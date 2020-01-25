# Settings explanation

## connection: client_ip

This is an IP address for the client (without the port). It can be a range of IPs. if omitted, any IP is allowed as client.

## connection: server_port

Address port used to identify the server during streaming. The handshake port is hardcoded to 9943.

## connection: client_port

Address port used to identify the client during streaming. The handshake port is hardcoded to 9943.

## connection: socket_desc: {...}

Please refer to [Laminar documentation](https://docs.rs/laminar/0.3.2/laminar/struct.Config.html).

Supported parameters:

* `"idle_connection_timeout_ms"`. Note 1: corresponding laminar field is `idle_connection_timeout`. Note 2: this should be comprehensive of the running start setup time.
* `"max_packet_size"`
* `"max_fragments"`
* `"fragment_size"`
* `"fragment_reassembly_buffer_size"`
* `"receive_buffer_max_size"`
* `"rtt_smoothing_factor"`
* `"rtt_max_value"`
* `"socket_event_buffer_size"`
* `"max_packets_in_flight"`

If any field is omitted, the default value is used.

## video: frame_size

This can be either:

* `{ "Scale": {n} }` (where `{n}` is a decimal number)
* `{ "Absolute": [{w}, {h}] }` (where `{w}` and `{h}` are the width and height of the frames)

## video: preferred_framerate

The client exposes some supported framerates. The server chooses the closest one to the specified by this option.

## video: composition_filtering

This controls the type of image filtering used by the BridgeVR compositor when the size of the layers does not match up with the choosen frame size. It can be:

* `"NearestNeighbour"`: This corresponds to no filtering. This can cause some visual artifacts.
* `"Bilinear"`: This is a basic filter that has no performance cost. Can cause the image to be blurry.
* `{ "Lanczos": {n} }`: This is best filter in terms of image quality. `{n}` is a smoothing factor. Please refer to this [wiki link](https://en.wikipedia.org/wiki/Lanczos_resampling).

## video: foveated_rendering

This can be either `{ "Enabled": { ... } }` or `"Disabled"`. If enabled:

* `"strength"`: strength of the foveation effect. 0 is equivalent to Disabled.
* `"shape_ratio"`: this controls the shape of the high quality region. A value between 1.5 an 2 should be used
* `"vertical_offset"`: this moves the high quality region vertically.

## video: frame_slice_count

Number of parts that the rendered images are subdivided into before being encoded and transmitted. A higher value reduces latency by parallelizing the computation workload. This number is restricted by the number of parallel instances of the video encoder that your GPU supports. On Nvidia GTX and RTX series the maximum number is 2 out of the box. You can find a patch for removing this restriction at [this page](https://github.com/keylase/nvidia-patch). Mind that this restriction is system-wide, so any screen recording software running can create issues.

## video: encoder

This must be set to `{ "FFmpeg": { "hardware_context": {x}, "config": { ... } }`.

`hardware_context` must be set according to your hardware:

* `"CUDA"` if using Nvidia GPU on Linux OS
* `"D3D11VA"` if using Windows

In `config` are the parameters passed to FFmpeg:

* `"codec_name"`: e.g. `h265_nvenc`
* `"context_options"`: general settings
* `"priv_data_options"`: settings relative to specific codec
* `"codec_open_options"`: same as (and in alternative to) `context_options` but formatted as in ffmpeg command line arguments
* `"frame_options"`: settings relative to frames
* `"hw_frames_context_options"`: codec specific frame settings

## video: decoder

Similar to the `"encoder"` settings but `hardware_context` must be set to:

* `"MediaCodec"` if using Oculus Quest or Oculus Go
* `"D3D11VA"` if using Windows Mixed Reality,

## video: buffering_frame_latency

BridgeVR uses basic statistical analysis to reduce latency by timing certain internal events. The latency controlled by these settigs is not the total latency of the streaming pipeline but only the last bit, where frames are ready to be displayed and are waiting for the display vsync.

`buffering_frame_latency` affects the motion-to-photon latency. This can be either `{ "Automatic": { ... } }` or `{ "Manual": { ... } }`.

Automatic:

* `"default_ms"`: initial estimation of the target latency. This has effect only the first time you use BridgeVR. The updated value is saved and loaded between sessions.
* `"expected_misses_per_hour"`: tolerable frequency of stutters caused by a missed deadline of an internal event. This is not guaranteed to be accurate because it assumes the latency samples to be distributed as in a [normal variable](https://en.wikipedia.org/wiki/Normal_distribution). You can lower this value but mind that if set too low it can increase the latency.
* `"history_mean_lifetime_s"`: the higher the value, the lower the judder, but higher the response time (to e.g. network speed change).

Manual:

* `"ms"`: same as `default_ms` but this value is not dynamically updated.
* `"history_mean_lifetime_s"`: same as for `Automatic`.

## video: buffering_head_pose_latency

Similar to `buffering_frame_latency`, but this controls the timing for reducing the "black pull" and the positional tracking latency. An high `history_mean_lifetime_s` value is recommended.

## video: reliable

Set to true to use [Laminar](https://github.com/amethyst/laminar) reliable mode for video packets (mode similar to TCP but over UDP). Enable this if you get bad video/audio glitches. This will increase latency.

## game_audio

This can be either `{ "Enabled": { ... } }` or `"Disabled"`.

## game_audio: Enabled: input_device_index

Index of audio input device on server. If omitted the default one is used.

## game_audio: Enabled: output_device_index

Index of audio output device on client. If omitted the default one is used.

## game_audio: Enabled: preferred_sample_rate

Sample rate for game audio stream. Used if both the client and the server supports it, otherwise use the default one.

## game_audio: Enabled: preferred_format

Audio format for game audio stream. Used if both the client and the server supports it, otherwise use the default one.

## game_audio: Enabled: reliable

Similar to `reliable` in video section.

## microphone

Similar to `game_audio` section but for microphone. `input_device_index` refers to the client, `output_device_index` refers to the server.

## openvr: server_idle_timeout_s

Time in seconds for the server to shutdown after SteamVR has started or after a client is disconnected.

## openvr: block_standby

Disable standby on server. This does disable standby on the headset. When the screen on the headset turns off, the client will always disconnect and the it will reconnect when you put the headset on.

## openvr: input_mapping

Its value is `[{left_controller_mapping}, {right_controller_mapping}]`.
Each of the two is formatted as: `[[{openvr_path}, {input_type}, [{client_path}, ... ]], ... ]`

`{openvr_path}` can be:

* `"/input/a/click"`
* `"/input/a/touch"`
* `"/input/b/click"`
* `"/input/b/touch"`
* `"/input/x/click"`
* `"/input/x/touch"`
* `"/input/y/click"`
* `"/input/y/touch"`
* `"/input/joystick/click"`
* `"/input/joystick/x"`
* `"/input/joystick/y"`
* `"/input/joystick/touch"`
* `"/input/trigger/click"`
* `"/input/trigger/value"`
* `"/input/trigger/touch"`
* `"/input/grip/click"`
* `"/input/grip/value"`
* `"/input/grip/touch"`
* `"/input/back/click"`
* `"/input/guide/click"`
* `"/input/start/click"`
* `"/input/system/click"`
* `"/input/application_menu/click"`
* todo: add missing touchpad

`{input_type}` can be:

* `"NormalizedOneSided"`: this is for value of triggers an grips
* `"NormalizedTwoSided"`: this is for thumbstick or touchpad x/y position
* `"Boolean"`: for the rest of openvr input types

`{openvr_path}` can be:

* `"/gamepad/left/joystick/x"`
* `"/gamepad/left/joystick/y"`
* todo: other in `input_mapping.rs`

## openvr: compositor_type

This can be either:

* `"Custom"`: (recommended) uses DirectModeDriver interface.  
  Cons:
  * supports a limited number of color formats.
  * there can be some glitches with head pose when OpenVR submits more than one layer.
* `"SteamVR"`: uses VirtualDisplay interface  
  Pro: none of Custom mode cons.  
  Cons:
  * tiny bit more latency
  * potential lower image quality

## openvr: preferred_render_eye_resolution

Set this to `[{w}, {h}]` (where `{w}` is width and `{h}` is height) when you want to use a different rendering resolution than the one dictated by the client native screen resolution. Recommended to leave unset.

## openvr: hmd_custom_properties

Collection of properties relative to the OpenVR HMD tracking device set on activation.

Format: `[{ "code": {n}, "value": {v} }, ... ]`

`code` is an integer corresponding to a property listed [here](https://github.com/ValveSoftware/openvr/blob/master/headers/openvr_driver.h#L307).

`value` is one of the following:

* `{ "Bool": {b} }`
* `{ "Int32": {n} }`
* `{ "Uint64": {n} }`
* `{ "Float": {n} }`
* `{ "String": {s} }`
* `{ "Vector3": [{n}, {n}, {n}] }`

## openvr: controllers_custom_properties

Collection of properties relative to the OpenVR controllers tracking devices set on activation.

Format: `[{left}, {right}]` where `{left}` and  `{right}` are settings similar to `hmd_custom_properties`

## headsets: untracked_default_controller_poses

This is the pose that the hands assume when they are not tracked. There is no way of removing them, but you can give them a large negative z value to hide them in the opposite direction of your gaze or a large negative y value to hide them under the ground.

Format: `[{left}, {right}]`.

Each of left and right contains:

* `"position": [{n}, {n}, {n}]`: corresponds to the position of the hand relative to the head when looking straight ahead.
* `"orientation": [{n}, {n}, {n}, {n}]`: a quaternion as the orientation of the hand relative to the head when looking straight ahead.

## headsets: head_height_3dof_meters

Height of the head relative to the ground. Applies only for 3DOF headsets.

## headsets: head_motion_model_3dof

For 3DOF headsets, use double integration of accelerometer data to infere an approximate position of the head. Can cause motion sickness and should be used ONLY IF SEATED.

Can be either `{ "Enable": { ... } }` or `"Disabled"`.

If Enabled:

* `"fix_threshold_meters_per_seconds_squared"`: fix head in place if the measure linear acceleration is below this threshold
* `"drift_threshold_radians_per_seconds"`: minimum head rotation speed before making the head drift back to default position, if below `fix_threshold_meters_per_seconds_squared`.
* `drift_speed_meters_per_second`: drift speed if above `drift_threshold_radians_per_seconds`.

## headsets: controllers_motion_model_3dof

Similar to `head_motion_model_3dof` but for the controllers.
