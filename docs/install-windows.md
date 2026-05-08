# CoreMQ — Windows Installation Guide

Tested on Windows 10 (22H2) and Windows 11. Run all commands in **PowerShell as Administrator**.

---

## Option A — Docker Desktop (recommended)

```powershell
# Install Docker Desktop: https://www.docker.com/products/docker-desktop/
# Enable WSL 2 backend when prompted.

git clone https://github.com/mqttRust/coremq-rust.git
cd coremq-rust

# Build the image
docker build -t coremq:latest .

# Run the container
docker run -d `
  --name coremq `
  --restart unless-stopped `
  -p 18083:18083 `
  -p 1883:1883 `
  -p 8083:8083 `
  -p 8883:8883 `
  -v coremq_data:/etc/coremq `
  -e COREMQ_CONFIG=/etc/coremq/config.yaml `
  coremq:latest
```

Open the dashboard: http://localhost:18083

Default credentials: `admin` / `public`

**Useful commands:**
```powershell
docker logs -f coremq        # follow logs
docker stop coremq           # stop
docker start coremq          # restart
docker rm coremq             # remove container
```

---

## Option B — Build from source

### Prerequisites

**Rust:**
```powershell
winget install Rustlang.Rustup
# Restart your terminal, then:
rustup default stable
```

**Node.js 20 + Yarn:**
```powershell
winget install OpenJS.NodeJS.LTS
npm install -g yarn
```

**Visual C++ Build Tools** (required for Rust native crates):
```powershell
winget install Microsoft.VisualStudio.2022.BuildTools
# Select "C++ build tools" workload during install
```

### Build

```powershell
git clone https://github.com/mqttRust/coremq-rust.git
cd coremq-rust

# Build React, then embed into the Rust binary
yarn --cwd client install
yarn --cwd client build
cargo build --release -p coremq-server
```

### Install config files

Config lives in `C:\ProgramData\CoreMQ\` on Windows:

```powershell
New-Item -ItemType Directory -Force -Path "C:\ProgramData\CoreMQ\data"
New-Item -ItemType Directory -Force -Path "C:\ProgramData\CoreMQ\tls"
Copy-Item server\coremq-server\config\config.yaml "C:\ProgramData\CoreMQ\config.yaml"
Copy-Item server\coremq-server\config\model.conf  "C:\ProgramData\CoreMQ\model.conf"
Copy-Item server\coremq-server\config\policy.csv  "C:\ProgramData\CoreMQ\policy.csv"
```

Edit the config:
```powershell
notepad "C:\ProgramData\CoreMQ\config.yaml"
```

Update the paths inside config.yaml:
```yaml
middleware:
  model_path: C:\ProgramData\CoreMQ\model.conf
  policy_path: C:\ProgramData\CoreMQ\policy.csv
  secret: change-this-to-a-long-random-secret
```

### Run

```powershell
$env:COREMQ_CONFIG = "C:\ProgramData\CoreMQ\config.yaml"
$env:COREMQ_DATA   = "C:\ProgramData\CoreMQ\data"
.\target\release\coremq-server.exe
```

Dashboard: http://localhost:18083

---

## Option C — Windows Service (production)

After building and confirming the binary works, register it as a Windows service:

```powershell
# Copy binary to a permanent location
Copy-Item .\target\release\coremq-server.exe "C:\Program Files\CoreMQ\coremq.exe"

# Register as a service
sc.exe create CoreMQ `
    binPath= '"C:\Program Files\CoreMQ\coremq.exe"' `
    start= auto `
    DisplayName= "CoreMQ MQTT Broker"

# Set environment variables for the service
reg add "HKLM\SYSTEM\CurrentControlSet\Services\CoreMQ\Environment" /v COREMQ_CONFIG /t REG_SZ /d "C:\ProgramData\CoreMQ\config.yaml" /f
reg add "HKLM\SYSTEM\CurrentControlSet\Services\CoreMQ\Environment" /v COREMQ_DATA   /t REG_SZ /d "C:\ProgramData\CoreMQ\data" /f

# Start the service
sc.exe start CoreMQ

# Check status
sc.exe query CoreMQ
```

Stop / remove:
```powershell
sc.exe stop CoreMQ
sc.exe delete CoreMQ
```

---

## Ports

| Port  | Protocol      | Description          |
|-------|---------------|----------------------|
| 18083 | HTTP          | Dashboard + REST API |
| 1883  | TCP           | MQTT                 |
| 8083  | WebSocket     | MQTT over WS         |
| 8883  | TLS           | Secure MQTT          |

### Windows Firewall rules

```powershell
New-NetFirewallRule -DisplayName "CoreMQ Dashboard" -Direction Inbound -Protocol TCP -LocalPort 18083 -Action Allow
New-NetFirewallRule -DisplayName "CoreMQ MQTT"      -Direction Inbound -Protocol TCP -LocalPort 1883  -Action Allow
New-NetFirewallRule -DisplayName "CoreMQ MQTT WS"   -Direction Inbound -Protocol TCP -LocalPort 8083  -Action Allow
```

---

## Data & Config locations

| Path                              | Description              |
|-----------------------------------|--------------------------|
| `C:\ProgramData\CoreMQ\config.yaml` | Main configuration     |
| `C:\ProgramData\CoreMQ\model.conf`  | Casbin RBAC model      |
| `C:\ProgramData\CoreMQ\policy.csv`  | Casbin access policy   |
| `C:\ProgramData\CoreMQ\data\`       | ReDB database          |
| `C:\ProgramData\CoreMQ\tls\`        | TLS certificates       |
