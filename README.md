# Roton Iced

Iced rewrite of Roton, a Wayland `wl-screenrec` wrapper focused on fast screen recording with a small custom UI.

## Features

- `wl-screenrec` recording backend
- MP4, MKV, and WEBM output
- fullscreen or `slurp` area recording
- monitor selection
- optional cursor capture
- optional screen sound capture
- optional microphone capture
- pause/resume by segmenting recordings and concatenating with `ffmpeg`
- recording completion notification
- custom titlebar with minimal-window controls
- tray icon and optional minimize-to-tray close behavior
- close confirmation while recording when minimize-to-tray is off

## Run

```sh
cargo run
```

## Runtime Tools

Install these on Arch:

```sh
sudo pacman -S ffmpeg slurp xdg-desktop-portal pipewire-pulse
```

Roton also expects `wl-screenrec` and `pactl` to be available. `wl-screenrec` is commonly installed from AUR.

## Notes

- This build targets Wayland.
- Audio devices are read from `pactl list sources`.
- Screen area selection uses `slurp`.
- Tray support uses StatusNotifier/AppIndicator through `tray-icon`.
