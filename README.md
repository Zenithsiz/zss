# zss

Scrolling wallpaper for X.

# Usage

Requires an existing X window, most commonly supplied by `xwinwrap`.

`xwinwrap -- zss WID <path-to-images-directory>`

See `zss --help` for other options, such as duration, fading and image backlog.


# Install

May be install using a nightly `cargo` with

```
cargo +nightly install zss
```

Or manually by cloning the repo

```
git clone https://github.com/Zenithsiz/zss
cd zss
cargo +nightly install --path .
```

Or

```
git clone https://github.com/Zenithsiz/zss
cd zss
cargo +nightly build --release
cp target/release/zss <install-path>
```

# Requirements

Requires `X` and opengl `3.3` at least (Not tested). Will attempt to use latest opengl available.


# Wallpaper

As per the description may be used as a wallpaper.

My settings are the following, use `xwinwrap` to see what each option does and to
adjust to your window geometry (I have 2 monitors, thus the large offsets).

```
xwinwrap -d -b -sp -nf -ov -g "1920x1080+1360+0" -- $(which zss) "WID" --duration "30" --images-dir "<my-images-dir>" --fade 0.95
```

Note that `which` is required here, as `xwinwrap` seems to require an absolute path.

This can be run once at start-up and the wallpapers will keep running.

# Performance

Performance wasn't a huge concern with this project, and there are some corners cut, including:

- Large images are fully loaded and only after resized, so they make take a while to load.
- Minimum image backlog is 3 images, due to design

Although on my particular system, the wallpaper CPU usage is ~0.25% most of the time, with about ~5..15% during loading, which
only takes a split second every time you switch wallpapers.