set -e

cargo build --release
#xwinwrap -b -sp -nf -ov -g "1360x768+0+312" -- $(pwd)/target/release/zss WID --duration 30.0 --images-dir ~/.wallpaper/active
xwinwrap -b -sp -nf -ov -g "1920x1080+1360+0" -- $(pwd)/target/release/zss WID --duration 1.0 --images-dir ~/.wallpaper/active