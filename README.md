# :zap: Noti

**Noti** is a lightweight desktop notification daemon for Wayland compositors. It offers a customizable and efficient notification experience, designed for modern Linux desktops.

## :star2: Features

| status | feature               |
| :----: | :-------------------- |
|   ✅   | Hot-reload            |
|   ✅   | CLI                   |
|   ✅   | Per-App configuration |
|   ✅   | Themes                |
|   ✅   | Idle                  |
|   ✅   | Custom layout         |
|   ✅   | Gradients             |
|   🚧   | `Do-Not-Disturb` mode |
|   🚧   | History               |
|   🚧   | Actions               |
|   ❌   | Audio                 |

## :rocket: Getting Started

The best way to get started with **Noti** is the [book](https://noti-rs.github.io/notibook).

## :inbox_tray: Installation

### Prerequisites

- Rust and Cargo installed ([rust-lang.org](https://www.rust-lang.org/tools/install))
- For extended installation: Nightly Rust (`rustup toolchain intall nightly`)

### Basic: Install via Cargo

```bash
# Install directly from GitHub
cargo install --git https://github.com/noti-rs/noti/

# Or clone and install locally
git clone https://github.com/noti-rs/noti.git
cd noti
cargo install --path .
```

### Extended: Minimal binary size (Nightly Only)

```bash
RUSTFLAGS="-Zlocation-detail=none -Zfmt-debug=none" cargo +nightly install -Z build-std=std,panic_abort -Z build-std-features="optimize_for_size" --target x86_64-unknown-linux-gnu --git https://github.com/noti-rs/noti
```

> [!IMPORTANT]
> The application uses `libc` allocator as default to minimize heap usage in runtime. You can turn off this option using `--no-default-features` flag.

## :hammer_and_wrench: Configuration

Noti uses a TOML configuration file located at:

- `$XDG_CONFIG_HOME/noti/config.toml`
- `~/.config/noti/config.toml`

**Example configuration**:

```toml
[general]
font = "JetBrainsMono Nerd Font"
anchor = "top-right"
offset = [15, 15]
gap = 10
sorting = "urgency"

width = 300
height = 150

[display]
theme = "pastel"
padding = 8
timeout = 2000

[display.border]
size = 4
radius = 10

[display.image]
max_size = 64
margin = { right = 25 }
# For old computers you can use simpler resizing method
# resizing_method = "nearest"

[display.text]
wrap = false
ellipsize_at = "middle"

[display.summary]
style = "bold italic"
margin = { top = 5 }
font_size = 18

[display.body]
justification = "left"
margin = { top = 12 }
font_size = 16

[[theme]]
name = "pastel"

[theme.normal]
background = "#1e1e2e"
foreground = "#99AEB3"
border = "#000"

[theme.critical]
background = "#EBA0AC"
foreground = "#1E1E2E"
border = "#000"

[[app]]
name = "Telegram Desktop"
[app.display]
border = { radius = 8 }
markup = true

[app.display.body]
justification = "center"
line_spacing = 5
```

> [!TIP]
> Check the [book](https://noti-rs.github.io/notibook) for comprehensive configuration guide!

### :wrench: Custom layout

Want to change the banner layout?
`Noti` offers a customizable layout using our file format, `.noti`!

Example of layout configuration:

```noti
FlexContainer(
    direction = vertical,
    spacing = Spacing(
        top = 10,
        right = 15,
        bottom = 10,
        left = 15,
    ),
    alignment = Alignment(
        horizontal = center,
        vertical = center,
    ),
    border = Border(
        size = 5,
        radius = 10,
    ),
) {
    Text(
        kind = summary,
        wrap = false,
        ellipsize_at = end,
        justification = center,
    )
    Text(
        kind = body,
        style = bold-italic,
        margin = Spacing(top = 12),
        justification = center,
    )
}
```

To enable this feature, write your own layout in file and in main config file write:

```toml
display.layout = "path/to/your/File.noti"
```

Read more about it [here](https://noti-rs.github.io/notibook/CustomLayout.html)!

## :bug: Troubleshooting

Having issues?

- Set the `NOTI_LOG` environment variable to `debug` or `trace` for detailed logs:

  ```bash
  NOTI_LOG=debug noti run >> debug.log
  ```

- Open a GitHub issue and attach the log file. This will help us resolve the problem faster.

## :handshake: Contributing

Interested in improving **Noti**? Here's how to contribute:

1. Fork the repo and create your branch:

   ```bash
   git checkout -b feature/my-improvement
   ```

2. Make your changes and commit them:

   ```bash
   git commit -am "feat: describe your changes"
   ```

3. Push your changes:

   ```bash
   git push origin feature/my-improvement
   ```

4. Open a Pull Request

> [!NOTE]
> For major changes, please open an issue first to discuss the changes you'd like to make.

## 📄 License

**Noti** is licensed under the GNU General Public License v3.0 (GPL-3.0).

This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.

See the [LICENSE](LICENSE) file for complete details.

## 📬 Contact

Have questions or need support? We're here to help! Open an issue on GitHub and we'll get back to you as soon as possible.
