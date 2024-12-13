# :zap: Noti

**Noti** is a lightweight desktop notification daemon for Wayland compositors. It offers a customizable and efficient notification experience, designed for modern Linux desktops.

## :star2: Features

- **Native Wayland Support**: Seamlessly integrated with Wayland
- **Highly Configurable**: Detailed customization options for appearance and behavior
- **Per-App Styling**: Unique notification styles for different applications
- **Modern Design**: Clean, minimalist approach to desktop notifications

## :inbox_tray: Installation

### 1. Install via Cargo (Recommended)

```bash
# Install directly from GitHub
cargo install --git https://github.com/noti-rs/noti/

# Or clone and install locally
git clone https://github.com/noti-rs/noti.git
cd noti
cargo install --path .
```

### 2. Build from Source

```bash
# Clone the repository
git clone https://github.com/noti-rs/noti.git
cd noti

# Build in release mode
cargo build --release

# Install the binary
cargo install --path .
```

## :rocket: Running Noti

### Manual Startup

```bash
# Start Noti daemon
noti run
```

### Automatic Startup

For detailed instructions on setting up automatic startup with `systemd`, please refer to the [Autostart Guide](docs/Autostart.md).

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
# For old computers you can use simplier resizing method
# resizing_method = "nearest"

[display.text]
wrap = false
ellipsize_at = "middle"

[display.title]
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
> Check ![ConfigProperties.md](docs/ConfigProperties.md) for comprehensive configuration options!

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
