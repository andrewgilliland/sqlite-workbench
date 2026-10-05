# SQLite Workbench

A small native desktop starter built with [GPUI Kit](https://gpui-kit.com/).

## Requirements

- Rust 1.95.0 with Cargo, selected automatically by `rust-toolchain.toml` when
  using rustup. Although GPUI Kit's installation guide lists Rust 1.92, its
  published GPUI dependency uses `std::hint::cold_path`, stabilized in 1.95.
- On macOS: macOS 15 or newer and Xcode Command Line Tools. GPUI's Metal
  shader build also needs a working Metal compiler in the selected Xcode installation.
- For other platforms, follow the [GPUI Kit installation guide](https://gpui-kit.com/docs/installation).

## Run

From this directory:

```sh
cargo run --locked
```

The first build downloads and compiles the UI framework and can take several minutes.
The window contains a header, database explorer sidebar, welcome panel, and status bar.
Click **Open Database** to see the placeholder status update. No file picker or SQLite
connection is implemented yet. Closing the window exits the application.

## Development checks

```sh
cargo fmt --check
cargo check --locked
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

There are no automated tests yet; this version has no database behavior. Smoke-test
the native UI by launching it, clicking the button, resizing the window, and closing it.

## Structure

- `src/main.rs`: framework initialization, assets, window creation, and app lifecycle.
- `src/shell.rs`: themed layout and persistent placeholder interaction state.

The application uses the styled component layer; "shell" refers to the desktop layout,
not the optional JavaScript extension system (`gpui-shell`).

## Next slices

1. Open a local SQLite file through a native file picker.
2. Inspect tables and views in the explorer.
3. Add a SQL editor and a virtualized results table.

Startup follows the [GPUI Kit getting-started example](https://gpui-kit.com/docs/getting-started).
GPUI Kit and its source examples are licensed under Apache-2.0.