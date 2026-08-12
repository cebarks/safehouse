# Deployment

Production deployment guide for safehouse on a dedicated Linux server.

## Building

```bash
# Release build (stripped, single codegen unit)
cargo build --release

# Binary is at target/release/safehouse (~12 MB)
cp target/release/safehouse /usr/local/bin/
```

## Initial Setup

```bash
# Create a dedicated user
sudo useradd -r -m -s /bin/bash pzserver

# Switch to that user
sudo -iu pzserver

# Install SteamCMD (Debian/Ubuntu)
sudo apt install steamcmd

# Run setup
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

Create `/etc/systemd/system/safehouse-server.service`:

```ini
[Unit]
Description=Project Zomboid Server (via Safehouse)
After=network.target

[Service]
Type=forking
User=pzserver
Group=pzserver
ExecStart=/usr/local/bin/safehouse server start
ExecStop=/usr/local/bin/safehouse server stop
PIDFile=/home/pzserver/.local/share/safehouse/run/server.pid
Restart=on-failure
RestartSec=30

[Install]
WantedBy=multi-user.target
```

Create `/etc/systemd/system/safehouse-web.service`:

```ini
[Unit]
Description=Safehouse Web UI
After=network.target

[Service]
Type=simple
User=pzserver
Group=pzserver
ExecStart=/usr/local/bin/safehouse serve
Restart=on-failure
RestartSec=10

[Install]
WantedBy=multi-user.target
```

Enable and start:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now safehouse-server
sudo systemctl enable --now safehouse-web
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

When using a TLS proxy, the session cookies are still secure because they're only transmitted between the browser and nginx over HTTPS. The nginx→safehouse hop is on localhost.

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
| 27015 | RCON | Admin commands — localhost only |

```bash
# UFW example
sudo ufw allow 16261:16262/udp
sudo ufw allow 443/tcp
sudo ufw enable
```

## Automated Backups

Add cron jobs under the `pzserver` user:

```bash
sudo -iu pzserver crontab -e
```

```cron
# Backup every 6 hours
0 */6 * * * /usr/local/bin/safehouse backup create --label "auto"

# Prune old backups daily at 4am
0 4 * * * /usr/local/bin/safehouse backup prune
```

## Monitoring

Check server status:

```bash
safehouse server status
systemctl status safehouse-server
systemctl status safehouse-web
```

View logs:

```bash
# PZ server logs
safehouse server logs --follow

# Safehouse service logs
journalctl -u safehouse-server -f
journalctl -u safehouse-web -f
```

## Updating PZ Server

SteamCMD can update the PZ server in-place:

```bash
# Stop the server first
safehouse server stop

# Re-run setup (downloads updates)
safehouse setup --install-dir ~/pzserver

# Start again
safehouse server start
```
