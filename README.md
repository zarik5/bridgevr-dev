# BridgeVR

todo: discord + paypal badge  
todo: add tags: VR, SteamVR, OpenVR, OpenXR, Linux, cross-platform

Play SteamVR games wirelessly on Oculus Quest and other VR headsets. Requires an high end PC (64 bit OS only) and 5Ghz Wi-Fi network.

For now, this software requires you to tinker with low level settings. If you want a similar but easier tool, you can look at [ALVR](https://github.com/JackD83/ALVR) (Windows only).

## Supported platforms

More information on the related [wiki page](todo link)

|         Operative system         | Client[1] | Server |
| :------------------------------: | :----: | :----: |
|     Windows 10 (Nvidia GPU)      |   ❗    |   ✔️   |
|       Windows 10 (AMD GPU)       |   ❗    |   ?    |
|            Windows 7             |   ❗    |   ?    |
|    Ubuntu 19.10 (Nvidia GPU)     |   ❌    |   ✔️   |
| Other Linux distros (Nvidia GPU) |   ❌    |   ?    |
|         Linux (AMD GPU)          |   ❌    |  ❌[2]  |
|              macOS               |   -    |  ❗/❌   |

|           VR headset            | Client |
| :-----------------------------: | :----: |
|          Oculus Quest           |   ✔️   |
|            Oculus Go            |  ?[3]  |
|             GearVR              |   ❌    |
| Windows Mixed Reality headsets  |   ❗    |
| Oculus wired headsets (Windows) |   ❗    |
|  Vive and other wired headsets  |   ❌    |

✔️: Supported (for some combination of OS/hardware)  
?: Unknown support status (require testing)  
❗: Could be supported with relatively little work  
❌: Could be supported with major work  
[1]: Client for a remote server or backpack PC solution  
[2]: Waiting on support by dependency (FFmpeg)  
[3]: Tested only on the simulator  

## Features

* Linux support
* Automatic video stream latency and judder minimization
* Sliced encoding and multisocket streaming
* Fixed foveated rendering

## Upcoming features

* Automatic SteamVR Chaperone setup
* Gamma correction
* Presets, differential settings saving/loading

Any help on supporting new features or platforms is gratly appreciated!

## Installation

Please check out the [wiki](todo link). It contains also guides for building from source and explanation on how BridgeVR works internally.

## Troubleshoot

If you have any problems or questions, please join us on our [Discord server](todo link)

## License

[MIT](todo link)

## Support the developement

[PayPal](todo link)

## Credits

Riccardo Zaglia (zarik5)

A big thanks to @poligraphene, @JackD83 and all contributors of [ALVR](https://github.com/JackD83/ALVR) for the research on optimizing the performance on Oculus Quest.  
