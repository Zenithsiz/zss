set -e

# Build
echo "Building"
cargo build --release

# Zss
ZSS=($(pwd)/target/release/zss "WID"
	~/.wallpaper/active \
	--duration 5.0 \
	--fade 0.8 \
	--backlog 0 \
)

# Start
echo "Starting"
#xwinwrap -b -sp -nf -ov -g "1360x768+0+312" -- ${ZSS[@]}
xwinwrap -b -sp -nf -ov -g "1920x1080+1360+0" -- ${ZSS[@]}
