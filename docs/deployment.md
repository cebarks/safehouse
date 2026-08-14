# Deployment

## Building

```bash
# Build safehouse binary
cargo install --path .

# Build the PZ server container image
podman build -t safehouse-pz -f Containerfile .
```

The release profile strips symbols and uses single codegen unit for a smaller binary.

## Initial Setup

```bash
# Create a dedicated user
sudo useradd -r -m -s /bin/bash pzserver

# Switch to that user
sudo -iu pzserver

# Enable podman socket for rootless containers
systemctl --user enable --now podman.socket

# Build the container image
podman build -t safehouse-pz -f Containerfile .

# Run setup (downloads PZ server into ~/pzserver)
safehouse setup --install-dir ~/pzserver

# Edit config
vim ~/.local/share/safehouse/safehouse.toml
```

Key settings to configure:

```toml
server_install_dir = "/home/pzserver/pzserver"
server_name = "myserver"
rcon_password = "a-strong-password-here"
web_bind = "127.0.0.1"    # Bind to localhost if using a reverse proxy
web_port = 9292
```

Set restrictive permissions on the config:

```bash
chmod 600 ~/.local/share/safehouse/safehouse.toml
```

## systemd Service

Create two service files — one for the PZ server container, one for the web UI:

```ini
# ~/.config/systemd/user/safehouse-server.service
[Unit]
Description=Safehouse PZ Server
After=network.target podman.socket

[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=%h/.cargo/bin/safehouse server start --timeout 180
ExecStop=%h/.cargo/bin/safehouse server stop

[Install]
WantedBy=default.target
```

```ini
# ~/.config/systemd/user/safehouse-web.service
[Unit]
Description=Safehouse Web UI
After=network.target

[Service]
Type=exec
ExecStart=%h/.cargo/bin/safehouse serve
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
```

Enable and start:

```bash
systemctl --user daemon-reload
systemctl --user enable --now safehouse-server
systemctl --user enable --now safehouse-web

# Enable lingering so services run without an active login session
loginctl enable-linger pzserver
```

## Reverse Proxy (nginx)

The web UI runs on plain HTTP. For production, put it behind nginx with TLS.

```nginx
server {
    listen 443 ssl http2;
    server_name pz.example.com;

    ssl_certificate     /etc/letsencrypt/live/pz.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/pz.example.com/privkey.pem;

    location / {
        proxy_pass http://127.0.0.1:9292;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}

server {
    listen 80;
    server_name pz.example.com;
    return 301 https://$host$request_uri;
}
```

## Firewall

Open only the ports you need:

| Port | Protocol | Purpose |
| ------ | ---------- | --------- |
| 16261 | UDP | PZ game traffic (players connect here) |
| 16262 | UDP | PZ game traffic (secondary) |
| 443 | TCP | HTTPS (nginx reverse proxy) |

Do **not** expose these ports publicly:

| Port | Purpose | Why |
|------|---------|-----|
| 9292 | Safehouse web UI | Use reverse proxy instead |
| 27015 | RCON | Admin commands — container binds to localhost only |

```bash
# UFW example
sudo ufw allow 16261:16262/udp
sudo ufw allow 443/tcp
sudo ufw enable
```

## Automated Backups

Use a cron job or systemd timer:

```bash
# crontab -e (as pzserver user)
0 */6 * * * /home/pzserver/.cargo/bin/safehouse backup create --label auto
0 3 * * 0   /home/pzserver/.cargo/bin/safehouse backup prune
```

## Monitoring

Check server status:

```bash
safehouse server status
podman ps --filter name=safehouse-pz
systemctl --user status safehouse-server
systemctl --user status safehouse-web
```

View logs:

```bash
# PZ server logs (from container)
safehouse server logs --follow

# Safehouse service logs
journalctl --user -u safehouse-server -f
journalctl --user -u safehouse-web -f
```

## Updating PZ Server

```bash
# Stop the server first
safehouse server stop

# Re-run setup (downloads updates via SteamCMD)
safehouse setup --install-dir ~/pzserver

# Start again
safehouse server start
```

## Updating the Container Image

When PZ updates change runtime dependencies, rebuild the container image:

```bash
safehouse server stop
podman build -t safehouse-pz -f Containerfile .
safehouse server start
```
