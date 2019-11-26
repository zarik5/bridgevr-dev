
If `expected_missed_frames_per_hour` is lowered too much the latency could skyrocket.
`packet_loss_bitrate_factor` (<1) is a multiplier to decrease the bitrate every time a packet is lost or is late.
`new_session_bitrate_factor` (>1) is a multiplier to increase the bitrate since last session (driver startup). This is useful if the bitrate has been lowered by packet losses the previous sessions.  
`moving_average_history_seconds` should be set depending on your network reliability. The higher the value the slower to correct the latency for lost packets, but if it is too small the system cannot reliably calculate the ideal latency and that can cause visual stutter.

BridgeVR does not require a common time reference between the server and the client and instead uses a dynamic approach resistant to packet loss from both server and client.

The client queries the time of the next vsync and sends it to the server alongside pose data.  
The server, when invoked with `GetTimeSinceLastVsync`, returns `(time of next client vsync) - 1/fps + vsync_offset`.
The server renders, encodes and sends the frames to the client, which decodes and composits (prerenders) them as soon as they are available (respecting the frame order).  
A loop for submitting finished frames polls for a certain frame. The time span `now - (the finish time of compositing)` is recorded and used to calculate the mean and the standard deviation across the last `frame_n = moving_average_history_seconds * fps` frames. Assuming the time spans follow a normal distribution calculate the expected mean as a function of `expected_missed_frames_per_hour`. Then send `(mean - expected mean) / frame_n` to the server which should add it to `vsync_offset`.  
If the requested frame is late (or the relative packet is lost) record it's time as if it has finished *now*, send `force IDR for next frame` and `lower the bitrate` messages to the server and discard any subsequent frame that is not a IDR. While this is not optimal (the missed frame detection could have happended before encoding and the "force-IDR" could be sent only if the packet never arrived) if multiple frames are lost, the moving average time gets skewed such that `vsync_offset` on the server grows, `Present` gets invoked earlier and so the frames are dispatched earlier. If the lost packets are not caused by bad timing but by bad wireless communication, this system becomes unstable and the latency grows until it reaches `max_latency_ms`.
