
todo add tags: VR, SteamVR, OpenVR, OpenXR, Linux, cross-platform

# BridgeVR

Play SteamVR games wirelessly on Oculus Quest and other VR headsets. Requires an high end PC (64 bit OS only) and 5Ghz Wi-Fi network.

For now, this software requires you to tinker with low level settings. If you want a similar tool but easier to use, you can check [ALVR](https://github.com/JackD83/ALVR) (Windows only).

This software is provided as is and I cannot guarantee technical support.

## Supported platforms

More information on the related [wiki page](todo link)

|         Operative system         | Client | Server |
| :------------------------------: | :----: | :----: |
|       Windows 10 (AMD GPU)       |   ❗    |   ?    |
|     Windows 10 (Nvidia GPU)      |   ❗    |   ✔️   |
|       Windows 7 (AMD GPU)        |   ❌    |   ?    |
|      Windows 7 (Nvidia GPU)      |   ❌    |   ?    |
|      Ubuntu 19.10 (AMD GPU)      |   ❗    |   ?    |
|    Ubuntu 19.10 (Nvidia GPU)     |   ❗    |   ✔️   |
|  Other Linux distros (AMD GPU)   |   ❗    |   ?    |
| Other Linux distros (Nvidia GPU) |   ❗    |   ?    |
|              macOS               |   ❌    |   ❗    |

|           VR headset           |   Client    |
| :----------------------------: | :---------: |
|          Oculus Quest          |     ✔️      |
|           Oculus Go            |    ?[2]     |
|             GearVR             |      ❗      |
|      Daydream smartphones      |      ❌      |
| Windows Mixed Reality headsets |      ❗      |
|     Oculus wired headsets      | ❗ (Windows) |
|         Varjo headsets         |      ❗      |
| Vive and other wired headsets  |  ❗ (Linux)  |

✔️: Supported (for some combination of OS/hardware)  
?: Unknown support status (require testing)  
❗: Could be supported with relatively little work  
❌: Could be supported with major work  
[1]: Upcoming support  
[2]: Tested only on the simulator or virtual machine  

Support of wired VR headsets could be useful for laptop-backpack solutions.

If you find that your OS/hardware is not supported you can propose support opening an issue.

## Features

* Linux support
* Automatic video stream latency and bitrate adjustment
* Sliced encoding
* Fixed foveated rendering

## Upcoming features

* GUI
* Audio streaming
* Microphone integration
* Multi socket streaming to increase bitrate on Oculus Quest
* Automatic SteamVR Chaperone setup
* Gamma correction
* Differential settings saving/loading

Any help on supporting new features or platforms is gratly appreciated!

## Installation

Please check out the [wiki](todo link). It contains also guides for building from source and explanation on how BridgeVR works internally.

## Credits

A big thanks to @poligraphene, @JackD83 and all contributors of [ALVR](https://github.com/JackD83/ALVR) for the research on optimizing the performance on Oculus Quest.
