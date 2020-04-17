# Build from source

## Setup environment

### Dependencies

* [rustup](https://rustup.rs/) (install default toolchain for your platform)
* LLVM & Clang
* Flutter SDK

additional dependencies for Linux:

* GTK 3

additional dependencies for Windows:

* MSVC compiler

### Flutter

Clone Flutter SDK from [master branch](https://github.com/flutter/flutter), add `C:\your\path\to\flutter\bin` to `Path` environment vartiable, then follow the guide [here](https://github.com/flutter/flutter/wiki/Desktop-shells)

### Build release

```sh
$ bash build_release.sh
```
