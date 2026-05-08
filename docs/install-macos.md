# CoreMQ — macOS Installation Guide

Tested on macOS 13 Ventura and 14 Sonoma (Apple Silicon + Intel).

---

## Option A — Docker (recommended)

```bash
# Install Docker Desktop if not already: https://www.docker.com/products/docker-desktop/

git clone https://github.com/mqttRust/coremq-rust.git
cd coremq-rust

# Build the image
docker build -t coremq:latest .

# Run the container
docker run -d \
  --name coremq \
  --restart unless-stopped \
  -p 18083:18083 \
  -p 1883:1883 \
  -p 8083:8083 \
  -p 8883:8883 \
  -v coremq_data:/etc/coremq \
  -e COREMQ_CONFIG=/etc/coremq/config.yaml \
  coremq:latest

open http://localhost:18083
```

Default credentials: `admin` / `public`

**Useful commands:**
```bash
docker logs -f coremq        # follow logs
docker stop coremq           # stop
docker start coremq          # restart
docker rm coremq             # remove container
```

---

## Option B — Build from source

### Prerequisites

```bash
# Xcode command-line tools
xcode-select --install

# Homebrew
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Node.js 20 + Yarn
brew install node@20
npm install -g yarn
```

### Build

```bash
git clone https://github.com/mqttRust/coremq-rust.git
cd coremq-rust

make build
make install-config     # copies config to /etc/coremq/
```

### Configure

```bash
sudo nano /etc/coremq/config.yaml
```

Change the JWT `secret` and adjust listeners as needed.

### Run

```bash
sudo coremq        # if installed to /usr/local/bin
# or
sudo ./target/release/coremq-server
```

Dashboard: http://localhost:18083

---

## Option C — launchd service (auto-start on login)

After completing Option B and copying the binary to `/usr/local/bin/coremq`:

```bash
sudo tee /Library/LaunchDaemons/io.coremq.broker.plist > /dev/null <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>io.coremq.broker</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/coremq</string>
    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>COREMQ_CONFIG</key>
        <string>/etc/coremq/config.yaml</string>
        <key>COREMQ_DATA</key>
        <string>/etc/coremq/data</string>
    </dict>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/var/log/coremq.log</string>
    <key>StandardErrorPath</key>
    <string>/var/log/coremq.log</string>
</dict>
</plist>
EOF

sudo launchctl load /Library/LaunchDaemons/io.coremq.broker.plist
sudo launchctl start io.coremq.broker
```

Check status:
```bash
sudo launchctl list | grep coremq
tail -f /var/log/coremq.log
```

Stop:
```bash
sudo launchctl stop io.coremq.broker
sudo launchctl unload /Library/LaunchDaemons/io.coremq.broker.plist
```

---

## Ports

| Port  | Protocol      | Description          |
|-------|---------------|----------------------|
| 18083 | HTTP          | Dashboard + REST API |
| 1883  | TCP           | MQTT                 |
| 8083  | WebSocket     | MQTT over WS         |
| 8883  | TLS           | Secure MQTT          |

---

## Data & Config locations

| Path                        | Description              |
|-----------------------------|--------------------------|
| `/etc/coremq/config.yaml`   | Main configuration       |
| `/etc/coremq/model.conf`    | Casbin RBAC model        |
| `/etc/coremq/policy.csv`    | Casbin access policy     |
| `/etc/coremq/data/`         | ReDB database            |
| `/etc/coremq/tls/`          | TLS certificates         |

> **Note:** On macOS, `/etc` is a symlink to `/private/etc`. Both paths work.

---

## macOS Firewall

If you see connection refused from other devices, allow the ports:

**System Settings → Network → Firewall → Firewall Options** → add `coremq` binary.

Or via terminal:
```bash
sudo /usr/libexec/ApplicationFirewall/socketfilterfw --add /usr/local/bin/coremq
sudo /usr/libexec/ApplicationFirewall/socketfilterfw --unblockapp /usr/local/bin/coremq
```
