set -e

cargo build --release --quiet
xwinwrap -b -sp -nf -ov -g "1360x768+0+312" -- ./target/release/zss WID