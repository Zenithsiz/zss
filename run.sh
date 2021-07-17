set -e

cargo build --quiet
xwinwrap -b -sp -nf -ov -g "1360x768+0+312" -- ./target/debug/zss WID