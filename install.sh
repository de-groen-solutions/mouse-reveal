#!/bin/bash

set -e

function resize_icons() {
    #!/bin/bash

    # Path to your original 1024x1024 png
    ORIGINAL_ICON_PATH="icon.png"

    # Icon sizes
    declare -a SIZES=("16" "32" "48" "256")

    # For each size, create a new resized icon
    for SIZE in "${SIZES[@]}"
    do
        [ -f icons/${SIZE}x${SIZE}/dgs-mouse-reveal.png ] && continue
        mkdir -p icons/${SIZE}x${SIZE}
        convert $ORIGINAL_ICON_PATH -resize ${SIZE}x${SIZE} icons/${SIZE}x${SIZE}/dgs-mouse-reveal.png
    done

}

resize_icons

cargo build --manifest-path Cargo.toml --release

sudo install -m 755 target/release/dgs-mouse-reveal /usr/local/bin
sudo install -m 644 dgs-mouse-reveal.desktop /usr/share/applications/dgs-mouse-reveal.desktop
sudo install -m 644 icons/16x16/dgs-mouse-reveal.png /usr/share/icons/hicolor/16x16/apps/dgs-mouse-reveal.png
sudo install -m 644 icons/32x32/dgs-mouse-reveal.png /usr/share/icons/hicolor/32x32/apps/dgs-mouse-reveal.png
sudo install -m 644 icons/48x48/dgs-mouse-reveal.png /usr/share/icons/hicolor/48x48/apps/dgs-mouse-reveal.png
sudo install -m 644 icons/256x256/dgs-mouse-reveal.png /usr/share/icons/hicolor/256x256/apps/dgs-mouse-reveal.png

sudo gtk-update-icon-cache /usr/share/icons/hicolor
