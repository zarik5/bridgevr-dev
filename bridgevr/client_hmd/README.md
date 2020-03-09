# Build instructions

## Ubuntu

Download and unzip the [NDK](https://developer.android.com/ndk/downloads)

Append to `~/.profile`:

```
export ANDROID_NDK_HOME='path/to/ndk/'
```

Append to `~/.cargo/config` (or create the file):

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
