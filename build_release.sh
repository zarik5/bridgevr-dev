if [[ "$OSTYPE" == "linux-gnu" ]]; then
    SOURCE_DRIVER_NAME="libbridgevr_server.so"
    DEST_DRIVER_NAME="driver_bridgevr.so"
    PLATFORM="linux64"
    BVR_INSTALL_ROOT="$HOME/.bridgevr"
fi

INSTALL_ROOT="$BVR_INSTALL_ROOT" cargo build --release

DRIVER_DEST_DIR="release/driver/bin/$PLATFORM"
mkdir -p "$DRIVER_DEST_DIR"
cp "target/release/$SOURCE_DRIVER_NAME" "$DRIVER_DEST_DIR/$DEST_DRIVER_NAME"
