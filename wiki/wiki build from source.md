# Build from source

## Setup environment

Look at the installation script to set environment variables and open firewall ports, or just install the latest version of BridgeVR.

### Dependencies

* [rustup](https://rustup.rs/) (install default toolchain for your platform)
* LLVM & Clang
* npm
* [gstreamer](https://gstreamer.freedesktop.org/documentation/installing/index.html?gi-language=c) (normal and devel packages)

additional dependencies for Linux:

* GTK 3

additional dependencies for Windows:

* MSVC compiler

### Build release

```sh
$ bash build_release.sh
```
