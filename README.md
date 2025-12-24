<img src="assets/rotonicon.png" width="100" height="100" alt="Roton Icon">


# Roton Screen Recorder

Roton is a `wl-screenrec` wrapper, it's a screen recorder app and use `rust` language.

## About

I want a quick performance screen recorder app. And then I found `wl-screenrec`, it is so fast that i uninstall that studio app. but it's run on cli, so i want a GUI for it. 

So I make this app. This app can record either fullscreen or select area. You can adjust the audio source too, like mute, screen, audio, or both.

## Run this project

1. Install Rust by following its [getting-started guide](https://www.rust-lang.org/learn/get-started).
   Once this is done, you should have the `rustc` compiler and the `cargo` build system installed in your `PATH`.
2. Clone this repository:
    ```
    git clone https://github.com/ferdinankurnian/roton.git
    cd roton
    ```
3. Build with `cargo`:
    ```
    cargo build
    ```
4. Run the application binary:
    ```
    cargo run
    ```

## Note

Btw, this app use dependencies like `slurp`, `ffmpeg`, `pactl`, and ofc `wl-screenrec`. 

And hey, this is for wayland only.