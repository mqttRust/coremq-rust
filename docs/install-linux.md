# CoreMQ — Linux Installation Guide

Tested on Ubuntu 22.04+ and Debian 12. Adapt paths for other distros.

---

## Option A — Docker (recommended)

```bash
# 1. Clone the repository
git clone https://github.com/mqttRust/coremq-rust.git
cd coremq-rust

# 2. Build and start
docker compose up -d

# 3. Open the dashboard
xdg-open http://localhost:18083
```

Default credentials: `admin` / `public`

To view logs:
```bash
docker compose logs -f coremq
```

---

## Option B — Build from source

### Prerequisites

```bash
# Rust (1.83+)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Node.js 20+ and Yarn
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt-get install -y nodejs
npm install -g yarn
```

### Build

```bash
git clone https://github.com/mqttRust/coremq-rust.git
cd coremq-rust

# Build React frontend then embed into Rust binary
make build

# Install config files to /etc/coremq/
make install-config
```

### Configure

```bash
sudo nano /etc/coremq/config.yaml
```

Change the JWT secret and adjust listeners as needed.

### Run

```bash
sudo ./target/release/coremq-server
```

Or install system-wide:

```bash
sudo cp target/release/coremq-server /usr/local/bin/coremq
sudo coremq
```

Dashboard: http://localhost:18083

---

## Option C — systemd service (production)

After completing Option B:

```bash
sudo tee /etc/systemd/system/coremq.service > /dev/null <<'EOF'
[Unit]
Description=CoreMQ MQTT Broker
After=network.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=/usr/local/bin/coremq
Restart=on-failure
RestartSec=5s
Environment=COREMQ_CONFIG=/etc/coremq/config.yaml
Environment=COREMQ_DATA=/etc/coremq/data

# Run as dedicated user (optional but recommended)
# User=coremq
# Group=coremq

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable --now coremq
sudo systemctl status coremq
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

---

## Firewall (ufw)

```bash
sudo ufw allow 18083/tcp
sudo ufw allow 1883/tcp
sudo ufw allow 8083/tcp
```
