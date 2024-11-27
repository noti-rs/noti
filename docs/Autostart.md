# Autostarting Noti with Systemd and D-Bus

This guide will help you configure autostart `Noti` using `systemd` user services and `D-Bus` activation.

> [!WARNING]
> Specific configurations might vary slightly depending on your distribution and desktop setup.

## Understanding Environment Variables

| Variable           | Default Path     | Description                           |
| ------------------ | ---------------- | ------------------------------------- |
| `$XDG_DATA_HOME`   | `~/.local/share` | User-specific data directory          |
| `$XDG_CONFIG_HOME` | `~/.config`      | User-specific configuration directory |

## Step 1: D-Bus Service Configuration

Create a `D-Bus service` to define how application should be launched:

- Create a new file at `$XDG_DATA_HOME/dbus-1/services/org.freedesktop.Notifications.service`
- Add the following:

```ini
[D-Bus Service]
Name=org.freedesktop.Notifications
Exec=%h/.cargo/bin/noti run
SystemdService=noti.service
```

## Step 2: Systemd User Service Configuration

Create a `systemd unit` to manage application's lifecycle:

- Create a new file at `$XDG_CONFIG_HOME/systemd/user/noti.service`
- Add the following:

```ini
[Unit]
Description=Noti Application
PartOf=graphical-session.target
After=graphical-session.target

[Service]
Type=dbus
BusName=org.freedesktop.Notifications
Environment=XDG_CONFIG_HOME=%h/.config
Environment=NOTI_LOG=info
ExecStart=%h/.cargo/bin/noti run
Restart=always

[Install]
WantedBy=default.target
```

### Unit Configuration Breakdown

| Configuration                     | Purpose                                                   |
| --------------------------------- | --------------------------------------------------------- |
| `PartOf=graphical-session.target` | Ensures the service is managed with the graphical session |
| `Type=dbus`                       | Enables D-Bus activation                                  |
| `Restart=always`                  | Automatically restarts on failure                         |
| `WantedBy=default.target`         | Enables autostart at user login                           |

## Step 3: Enable and Start the Service

```bash
# Reload systemd user configuration
systemctl --user daemon-reload

# Enable service to start on boot
systemctl --user enable noti.service

# Start service immediately
systemctl --user start noti.service
```

## Troubleshooting

### Checking Service Status

```bash
# View service status
systemctl --user status noti

# Follow live service logs
journalctl --user --unit noti --follow
```

### Common Issues

- Ensure the executable path is correct
- Check file permissions
- Verify `D-Bus` and `systemd` configurations
- Confirm environment variables are set correctly
