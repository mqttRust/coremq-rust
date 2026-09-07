use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::future::BoxFuture;
use tokio::net::{UdpSocket, lookup_host};
use tokio::sync::RwLock;

use crate::models::config::cluster::{DiscoveryConfig, DiscoveryType};

/*
  Discovery produces *candidate addresses* only. It never decides membership —
  that is the failure detector's job. This split is what makes a stale DNS
  record or a departed seed harmless: dialling a dead address just fails.
*/
pub trait Discovery: Send + Sync {
    fn name(&self) -> &'static str;
    fn discover(&self) -> BoxFuture<'_, anyhow::Result<Vec<SocketAddr>>>;
}

pub fn build(cfg: &DiscoveryConfig, advertise: SocketAddr) -> anyhow::Result<Arc<dyn Discovery>> {
    let backend: Arc<dyn Discovery> = match cfg.kind {
        DiscoveryType::Static => Arc::new(StaticDiscovery {
            seeds: cfg.seeds.clone(),
        }),
        DiscoveryType::Dns => {
            if cfg.query.trim().is_empty() {
                anyhow::bail!("dns discovery requires cluster.discovery.query");
            }
            Arc::new(DnsDiscovery {
                query: cfg.query.clone(),
                port: cfg.port,
            })
        }
        DiscoveryType::Multicast => Arc::new(MulticastDiscovery::spawn(cfg, advertise)?),
        DiscoveryType::K8s => Arc::new(K8sDiscovery::from_config(cfg)?),
    };

    Ok(backend)
}

/*
  Seeds are a bootstrap hint, not a membership list. Once a node has learned the
  cluster through gossip it stays in it even if every configured seed is gone.
*/
pub struct StaticDiscovery {
    seeds: Vec<String>,
}

impl Discovery for StaticDiscovery {
    fn name(&self) -> &'static str {
        "static"
    }

    fn discover(&self) -> BoxFuture<'_, anyhow::Result<Vec<SocketAddr>>> {
        Box::pin(async move {
            let mut out = Vec::new();
            for seed in &self.seeds {
                match lookup_host(seed.as_str()).await {
                    Ok(addrs) => out.extend(addrs),
                    Err(e) => println!("cluster: seed '{}' did not resolve: {}", seed, e),
                }
            }
            Ok(out)
        })
    }
}

/*
  Resolves every A/AAAA record behind one name. A Kubernetes headless service and
  a Docker Compose service name both expose their pod set this way.

  SRV records are not supported — that needs a real resolver dependency, and the
  A/AAAA form covers both target environments.
*/
pub struct DnsDiscovery {
    query: String,
    port: u16,
}

impl Discovery for DnsDiscovery {
    fn name(&self) -> &'static str {
        "dns"
    }

    fn discover(&self) -> BoxFuture<'_, anyhow::Result<Vec<SocketAddr>>> {
        Box::pin(async move {
            let target = format!("{}:{}", self.query, self.port);
            let addrs = lookup_host(target.as_str()).await?;
            Ok(addrs.collect())
        })
    }
}

/*
  LAN autodiscovery over UDP multicast.

  This is a lightweight beacon rather than true mDNS/DNS-SD: each node
  periodically multicasts its own peer address and records the addresses it
  hears. That gives zero-config discovery on a flat network without pulling in
  an mDNS dependency, at the cost of not being discoverable by generic mDNS
  browsers. It does not traverse most cloud VPCs, which block multicast.
*/
pub struct MulticastDiscovery {
    seen: Arc<RwLock<HashMap<SocketAddr, Instant>>>,
    ttl: Duration,
}

impl MulticastDiscovery {
    fn spawn(cfg: &DiscoveryConfig, advertise: SocketAddr) -> anyhow::Result<Self> {
        let group: Ipv4Addr = cfg
            .group
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid multicast group '{}'", cfg.group))?;

        if !group.is_multicast() {
            anyhow::bail!("'{}' is not a multicast address", cfg.group);
        }

        let seen: Arc<RwLock<HashMap<SocketAddr, Instant>>> = Arc::new(RwLock::new(HashMap::new()));
        let port = cfg.multicast_port;
        let interval = cfg.interval;

        /*
          An entry is forgotten after a few missed beacons. Discovery output is
          only a dial hint, so being slightly stale costs one failed connect.
        */
        let ttl = interval * 6;

        let task_seen = seen.clone();
        tokio::spawn(async move {
            if let Err(e) = multicast_loop(group, port, advertise, interval, task_seen).await {
                println!("cluster: multicast discovery stopped: {}", e);
            }
        });

        Ok(Self { seen, ttl })
    }
}

async fn multicast_loop(
    group: Ipv4Addr,
    port: u16,
    advertise: SocketAddr,
    interval: Duration,
    seen: Arc<RwLock<HashMap<SocketAddr, Instant>>>,
) -> anyhow::Result<()> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port)).await?;
    socket.join_multicast_v4(group, Ipv4Addr::UNSPECIFIED)?;
    socket.set_multicast_loop_v4(false)?;

    let beacon = format!("coremq:{}", advertise);
    let target = SocketAddr::new(IpAddr::V4(group), port);
    let mut ticker = tokio::time::interval(interval);
    let mut buf = vec![0u8; 512];

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if let Err(e) = socket.send_to(beacon.as_bytes(), target).await {
                    println!("cluster: multicast beacon failed: {}", e);
                }
            }
            res = socket.recv_from(&mut buf) => {
                let Ok((len, _from)) = res else { continue };
                let Ok(text) = std::str::from_utf8(&buf[..len]) else { continue };
                let Some(addr_str) = text.strip_prefix("coremq:") else { continue };
                let Ok(addr) = addr_str.trim().parse::<SocketAddr>() else { continue };

                if addr == advertise {
                    continue;
                }
                seen.write().await.insert(addr, Instant::now());
            }
        }
    }
}

impl Discovery for MulticastDiscovery {
    fn name(&self) -> &'static str {
        "multicast"
    }

    fn discover(&self) -> BoxFuture<'_, anyhow::Result<Vec<SocketAddr>>> {
        Box::pin(async move {
            let now = Instant::now();
            let mut guard = self.seen.write().await;
            guard.retain(|_, last| now.duration_since(*last) < self.ttl);
            Ok(guard.keys().copied().collect())
        })
    }
}

/*
  Lists pods via the Kubernetes API using the pod's own service account.

  Uses the reqwest client already in the dependency graph rather than kube-rs;
  the query is a single filtered GET and does not justify the extra tree.
*/
pub struct K8sDiscovery {
    api_server: String,
    namespace: String,
    label_selector: String,
    token: String,
    port: u16,
    client: reqwest::Client,
}

const SA_DIR: &str = "/var/run/secrets/kubernetes.io/serviceaccount";

impl K8sDiscovery {
    fn from_config(cfg: &DiscoveryConfig) -> anyhow::Result<Self> {
        let token = std::fs::read_to_string(format!("{}/token", SA_DIR))
            .map_err(|e| anyhow::anyhow!("k8s discovery needs a service account token: {}", e))?;

        let namespace = if cfg.namespace.trim().is_empty() {
            std::fs::read_to_string(format!("{}/namespace", SA_DIR))
                .map_err(|e| anyhow::anyhow!("cannot determine namespace: {}", e))?
        } else {
            cfg.namespace.clone()
        };

        let ca_path = format!("{}/ca.crt", SA_DIR);
        let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(5));

        if let Ok(pem) = std::fs::read(&ca_path) {
            match reqwest::Certificate::from_pem(&pem) {
                Ok(cert) => builder = builder.add_root_certificate(cert),
                Err(e) => println!("cluster: ignoring unusable k8s CA cert: {}", e),
            }
        }

        let host = std::env::var("KUBERNETES_SERVICE_HOST")
            .map_err(|_| anyhow::anyhow!("KUBERNETES_SERVICE_HOST is not set"))?;
        let port = std::env::var("KUBERNETES_SERVICE_PORT").unwrap_or_else(|_| "443".to_string());

        Ok(Self {
            api_server: format!("https://{}:{}", host, port),
            namespace: namespace.trim().to_string(),
            label_selector: cfg.label_selector.clone(),
            token: token.trim().to_string(),
            port: cfg.port,
            client: builder.build()?,
        })
    }
}

impl Discovery for K8sDiscovery {
    fn name(&self) -> &'static str {
        "k8s"
    }

    fn discover(&self) -> BoxFuture<'_, anyhow::Result<Vec<SocketAddr>>> {
        Box::pin(async move {
            let url = format!("{}/api/v1/namespaces/{}/pods", self.api_server, self.namespace);

            let mut req = self.client.get(&url).bearer_auth(&self.token);
            if !self.label_selector.trim().is_empty() {
                req = req.query(&[("labelSelector", self.label_selector.as_str())]);
            }

            let res = req.send().await?;
            if !res.status().is_success() {
                anyhow::bail!("kubernetes API returned {}", res.status());
            }

            let body: serde_json::Value = res.json().await?;
            let mut out = Vec::new();

            for item in body["items"].as_array().unwrap_or(&Vec::new()) {
                /*
                  Only Running pods are dialable; a Pending pod has no IP and a
                  Terminating one is about to lose it.
                */
                if item["status"]["phase"].as_str() != Some("Running") {
                    continue;
                }
                let Some(ip) = item["status"]["podIP"].as_str() else {
                    continue;
                };
                let Ok(parsed) = ip.parse::<IpAddr>() else {
                    continue;
                };
                out.push(SocketAddr::new(parsed, self.port));
            }

            Ok(out)
        })
    }
}
