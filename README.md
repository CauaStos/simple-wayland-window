# simple-wayland-window

A minimal Wayland client window in Rust, based on the [`simple_window.rs`](https://github.com/Smithay/wayland-rs/blob/master/wayland-client/examples/simple_window.rs) example from the [`wayland-client`](https://docs.rs/wayland-client) crate.

This version includes **personal comments and notes** explaining various parts of the code, intended as a study resource for understanding how basic Wayland clients work in Rust.

## Purpose

This repo is primarily for **learning and reference**. It’s a near-direct copy of the original example with inline commentary to help clarify the flow, objects involved, and Wayland protocol concepts.

## What It Does

- Connects to a Wayland compositor  
- Sets up a surface and shell surface  
- Displays, for now, a 320x240 gradient

## Why This Exists

The official example is great but lacks inline explanation. This version breaks it down with comments for educational purposes. If you're new to Wayland or want to see what’s going on under the hood, this may help.

## Build & Run

```sh
cargo build --release
cargo run
```

Make sure you're running under a Wayland session (Hyprland, Sway, etc.).

## License

MIT — includes content derived from the `wayland-rs` examples, which are also MIT licensed.
