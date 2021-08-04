set -e

cargo build --release
xwinwrap -b -sp -nf -ov -g "1360x768+0+312" -- $(pwd)/target/release/zss WID ~/.wallpaper/active --duration 5.0 --fade 0.8 --backlog 0
#xwinwrap -b -sp -nf -ov -g "1920x1080+1360+0" -- $(pwd)/target/release/zss WID ~/.wallpaper/active --duration 5.0 --fade 0.8 --backlog 0
