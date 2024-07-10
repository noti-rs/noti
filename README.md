# Noti

Noti is a modern, lightweight notification manager designed for the Wayland display server protocol. It aims to provide a seamless and efficient notification experience for Wayland users.

## Features

- **Wayland Support**: Fully compatible with Wayland, ensuring smooth operation on modern Linux desktops.
- **Lightweight**: Minimal resource usage for efficient performance.
- **Customizable**: Easily configurable to fit your notification style preferences
- **Custom Configurations:** Supports per-application custom configurations with a mergeable settings system.

## Installation
### Building from Source

1. Clone the repository:
    ```bash
    git clone https://github.com/noti-rs/noti.git
    cd noti
    ```

2. Build the project using Cargo:
    ```bash
    cargo build --release
    ```

3. Install the binary:
    ```bash
    sudo cp target/release/noti /usr/local/bin/
    ```

### Running Noti

After installation, you can start Noti with:
```bash
noti
```

To enable Noti to start automatically with your Wayland session, add it to your session startup script.

## Configuration

Noti can be configured via a configuration file located at `~/.config/noti/config.toml`. Below is an example configuration:
```toml
[general]
timeout = 2000

[display]
font = [ "JetBrainsMono Nerd", 10 ]
width = 300
height = 100

[display.colors.normal]
background = "#1e1e2e"
foreground = "#99AEB3"

[display.colors.critical]
background = "#EBA0AC"
foreground = "#1E1E2E"

[[app]]
name = "Telegram"
[display]
rounding = 8
markup = true
```

## Contributing

Contributions are welcome! Please fork the repository and submit a pull request for any changes. For major changes, please open an issue first to discuss what you would like to change
- Fork the repository
- Create your feature branch: `git checkout -b your-feature`
- Commit your changes: `git commit -am 'Add some feature'`
- Push to the branch: `git push origin your-feature`
- Create a new Pull Request

## Contact

For any inquiries or support, please open an issue on GitHub.
