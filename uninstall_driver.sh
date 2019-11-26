if [[ "$OSTYPE" == "linux-gnu" ]]; then
    PLATFORM="linux64"
    BVR_INSTALL_ROOT="$HOME/.bridgevr"
fi

STEAMVR_BIN_DIR="$HOME/.steam/steam/steamapps/common/SteamVR/bin/$PLATFORM"
LD_LIBRARY_PATH="$STEAMVR_BIN_DIR" "$STEAMVR_BIN_DIR/vrpathreg" removedriver "$BVR_INSTALL_ROOT"

rm -rf "$BVR_INSTALL_ROOT"
