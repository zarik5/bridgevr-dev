# BridgeVR

todo: discord + paypal badge  
todo: add tags: VR, SteamVR, OpenVR, OpenXR, Linux, cross-platform

Play SteamVR games wirelessly on Oculus Quest and other VR headsets. Requires an high end PC (64 bit OS) and 5Ghz Wi-Fi network.

For now, this software requires you to tinker with low level settings. If you want a similar but easier tool, you can look at [ALVR](https://github.com/JackD83/ALVR) (Windows only).

## Supported platforms

More information on the related [wiki page](todo link)

|         Operative system         | Server support |
| :------------------------------: | :------------: |
|     Windows 10 (Nvidia GPU)      |       ✔️       |
|       Windows 10 (AMD GPU)       |       ?        |
|            Windows 7             |       ?        |
|    Ubuntu 19.10 (Nvidia GPU)     |       ✔️       |
| Other Linux distros (Nvidia GPU) |       ?        |
|         Linux (AMD GPU)          |     ❌ [1]      |
|              macOS               |      ❗/❌       |

|            VR headset             | Client support |
| :-------------------------------: | :------------: |
|           Oculus Quest            |       ✔️       |
|             Oculus Go             |     ? [2]      |
|              GearVR               |       ❌        |
|  Windows Mixed Reality headsets   |   ? [3] [4]    |
|  Oculus wired headsets (Windows)  |       ❗        |
| HTC Vive and other wired headsets |       ❌        |

✔️: Supported (for some combination of OS/hardware)  
?: Unknown support status (requires testing)  
❗: Could be supported with relatively little work  
❌: Could be supported with major developement work  
[1]: Waiting on support by dependency (FFmpeg). [Article link](https://www.phoronix.com/scan.php?page=news_item&px=FFmpeg-AMD-AMF-Vulkan)  
[2]: Tested on Oculus Quest  
[3]: Tested on the simulator  
[4]: Client to connect to a remote server or for a backpack PC solution

## Features

* Linux support
* Automatic video stream latency and judder minimization
* Sliced encoding and multisocket streaming [todo remove?](https://discordapp.com/channels/564087419918483486/588170196968013845/644523694051426352) / todo synchronize and reorder sends by buffer size?
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

Code by Riccardo Zaglia @zarik5

A big thanks to @polygraphene, @JackD83 and all contributors of ALVR for the research on optimizing the performance on Oculus Quest.  
