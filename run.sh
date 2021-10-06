set -e

# Build
echo "Building"
cargo build --release

# Zss
ZSS=($(pwd)/target/release/zss "WID"
	~/.wallpaper/active \
	--duration 5.0 \
	--fade 0.8 \
	--backlog 4 \
	--grid 2x2 \
)

# Start
echo "Starting"
cargo run --release --quiet -- \
	~/.wallpaper/active \
	--window-geometry 1920x1080+1360+0 \
	--duration 15.0 \
	--fade 0.8 \
#	--backlog 4 \
#	--grid 2x2