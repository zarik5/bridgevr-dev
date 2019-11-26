if [[ "$OSTYPE" == "linux-gnu" ]]; then
    PLATFORM="linux64"
    BVR_INSTALL_ROOT="$HOME/.bridgevr"
fi

mkdir -p "$BVR_INSTALL_ROOT"
cp -a release/driver/. "$BVR_INSTALL_ROOT"

STEAMVR_BIN_DIR="$HOME/.steam/steam/steamapps/common/SteamVR/bin/$PLATFORM"
LD_LIBRARY_PATH="$STEAMVR_BIN_DIR" "$STEAMVR_BIN_DIR/vrpathreg" adddriver "$BVR_INSTALL_ROOT"

# todo: add environment variables, open ports install dependencies (gstreamer)