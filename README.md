# Roton Iced

Iced rewrite of Roton, a Wayland `wl-screenrec` wrapper focused on fast screen recording with a small custom UI.

## Features

- `wl-screenrec` recording backend
- MP4, MKV, and WEBM output
- fullscreen or `scrop` area recording
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

## Development

Install the file watcher:

```sh
sudo pacman -S watchexec
```

Run Roton with rebuild-and-relaunch on save:

```sh
make dev
```

## Runtime Tools

Roton needs these tools at runtime:

- `ffmpeg`
- `xdg-desktop-portal`
- `pipewire-pulse`
- `wl-screenrec`
- `scrop`
- `pactl`

Tray support also needs an AppIndicator runtime:

- `libayatana-appindicator3` or `libappindicator3`

## Notes

- This build targets Wayland.
- Audio devices are read from `pactl list sources`.
- Screen area selection uses `scrop`.
- Tray support uses StatusNotifier/AppIndicator through `tray-icon`.
