# Build instructions

## Install Android SDK and NDK

The easiest way is to install Android Studio (I had some problems using the command line tools).  
By default the latest android SDK is installed.  
The NDK is installed by going to Configure -> SDK Manager -> SDK Tools, select NDK then Apply.

Export `ANDROID_HOME` and `NDK_HOME` environment variables:

* On Linux, append to `~/.profile`:

    ```
    export ANDROID_HOME="$HOME/Android/Sdk"
    export NDK_HOME="$HOME/Android/Sdk/ndk/<version>/"
    ```

* On Windows, on environment variables set `ANDROID_HOME=C:\Users\<user>\Android\Sdk\ndk\<version>`

(You need to replace `<...>` accordingly)

## Setup cargo for android

Append to `~/.cargo/config` (or `C:\Users\<user>\.cargo\config`) (or create the file):

```
[target.aarch64-linux-android]
ar = "$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android-ar"
linker = "$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android24-clang++"
```

### Fix for backtrace crate

Append to `~/.profile`:

```
export PATH="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin:$PATH"
```

Inside NDK folder, in `toolchains/llvm/prebuilt/linux-x86_64/bin`, execute:

```
ln -s aarch64-linux-android24-clang aarch64-linux-android-clang
```
