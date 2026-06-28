//! 局域网设备发现（mDNS 注册+浏览 + UDP 广播+监听）。
//!
//! 架构：
//! - `Announcer` 在启动时创建**唯一**的 `ServiceDaemon`
//! - 注册和浏览复用同一个 daemon（不再每次创建新的）
//! - 停止时：`daemon.shutdown()` 干净退出 daemon 线程
//! - mDNS daemon 线程随 `Announcer` 生命周期创建和销毁

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use mdns_sd::ServiceDaemon;
use tauri::AppHandle;
use tokio::net::UdpSocket;
use tokio::sync::Notify;
use tracing::{debug, info, warn};

use super::messages::{
    DeviceIdentity, DiscoveryMessage, PeerInfo, MDNS_SERVICE_TYPE, TXT_DATA_VERSION,
    TXT_DEVICE_ID, TXT_DEVICE_NAME, TXT_FILE_COUNT, TXT_INSTANCE_ID, UDP_DISCOVERY_PORT,
};

// ─── Announcer（宣告自己存在）─────────────────────────────────

/// mDNS + UDP 宣告句柄，持有 daemon 贯穿整个服务生命周期。
pub struct Announcer {
    /// mDNS daemon（整个服务生命周期内唯一）。
    pub(super) daemon: ServiceDaemon,
    /// UDP 监听器关闭信号
    udp_shutdown: Arc<Notify>,
    /// 防止 double-stop（显式 stop + Drop 各调用一次）
    stopped: AtomicBool,
}

impl Announcer {
    /// 启动宣告：创建 mDNS daemon + 注册服务 + UDP 监听。
    pub async fn start(
        identity: &DeviceIdentity,
        instance_id: &str,
        port: u16,
    ) -> Result<Self, String> {
        let local_ips = get_local_ips()?;
        let local_ip = local_ips
            .first()
            .cloned()
            .unwrap_or_else(|| "127.0.0.1".to_string());

        // ─── 创建 mDNS daemon（唯一一个，贯穿服务生命周期）───
        let daemon = tokio::task::spawn_blocking(|| {
            ServiceDaemon::new().map_err(|e| format!("创建 mDNS daemon 失败: {e}"))
        })
        .await
        .map_err(|e| format!("spawn_blocking 失败: {e}"))??;

        // ─── 用这个 daemon 注册 mDNS 服务 ────────────────────
        let mdns_name = register_mdns(&daemon, identity, instance_id, port, &local_ip).await?;

        // ─── UDP 监听器 ─────────────────────────────────────
        let udp_shutdown = start_udp_listener(identity, instance_id, port, &local_ip).await?;

        info!(
            "宣告服务已启动: mDNS={:?}, UDP=:{}",
            mdns_name, UDP_DISCOVERY_PORT
        );

        Ok(Self { daemon, udp_shutdown, stopped: AtomicBool::new(false) })
    }

    /// 停止宣告（UDP 监听器 + mDNS daemon 干净关闭）。幂等。
    pub fn stop(&self) {
        if self.stopped.swap(true, Ordering::SeqCst) {
            return; // 已经停止过了
        }
        self.udp_shutdown.notify_one();
        // 干净关闭 mDNS daemon 线程
        match self.daemon.shutdown() {
            Ok(_receiver) => {
                // receiver 会在 daemon 线程退出时收到 Shutdown 状态，
                // 不等待以免阻塞，daemon 线程会在几百毫秒内自行退出。
                debug!("mDNS daemon shutdown 命令已发送");
            }
            Err(e) => warn!("mDNS daemon shutdown 发送失败: {e}"),
        }
    }
}

impl Drop for Announcer {
    fn drop(&mut self) {
        self.stop();
    }
}

// ─── mDNS 注册 ───────────────────────────────────────────────

async fn register_mdns(
    daemon: &ServiceDaemon,
    identity: &DeviceIdentity,
    instance_id: &str,
    port: u16,
    local_ip: &str,
) -> Result<Option<String>, String> {
    let device_id = identity.device_id.clone();
    let device_name = identity.device_name.clone();
    let instance_id = instance_id.to_string();
    let ip = local_ip.to_string();
    let d = daemon.clone();

    tokio::task::spawn_blocking(move || {
        // 构建服务名（含随机后缀避免冲突）
        let service_name = format!("{}_{}", device_name, &instance_id[..8]);

        let service_type = MDNS_SERVICE_TYPE.to_string();

        let mut props: HashMap<String, String> = HashMap::new();
        props.insert(TXT_DEVICE_ID.to_string(), device_id.clone());
        props.insert(TXT_INSTANCE_ID.to_string(), instance_id.clone());
        props.insert(TXT_DEVICE_NAME.to_string(), device_name.clone());
        props.insert(TXT_DATA_VERSION.to_string(), "0".to_string());
        props.insert(TXT_FILE_COUNT.to_string(), "0".to_string());

        let hostname = format!("{}.local.", device_name);

        let service_info = mdns_sd::ServiceInfo::new(
            &service_type,
            &service_name,
            &hostname,
            ip.as_str(),
            port,
            props,
        )
        .map_err(|e| format!("构造 mDNS 服务信息失败: {e}"))?;

        let fullname = service_info.get_fullname().to_string();

        d.register(service_info)
            .map_err(|e| format!("注册 mDNS 服务失败: {e}"))?;

        info!("mDNS 已注册: {}", fullname);
        Ok::<_, String>(Some(fullname))
    })
    .await
    .map_err(|e| format!("mDNS 注册任务失败: {e}"))?
}

// ─── UDP 监听器（响应探测）─────────────────────────────────────

async fn start_udp_listener(
    identity: &DeviceIdentity,
    instance_id: &str,
    port: u16,
    local_ip: &str,
) -> Result<Arc<Notify>, String> {
    let shutdown = Arc::new(Notify::new());
    let shutdown_clone = shutdown.clone();

    let msg = DiscoveryMessage {
        msg_type: "lingchat-response".to_string(),
        device_id: identity.device_id.clone(),
        instance_id: instance_id.to_string(),
        device_name: identity.device_name.clone(),
        host: local_ip.to_string(),
        port,
        data_version: 0,
        file_count: 0,
    };
    let response_json = serde_json::to_vec(&msg).map_err(|e| format!("序列化失败: {e}"))?;
    let response_data = Arc::new(response_json);

    let bind_addr = format!("0.0.0.0:{}", UDP_DISCOVERY_PORT);

    let socket = match UdpSocket::bind(&bind_addr).await {
        Ok(s) => s,
        Err(e) => {
            warn!("无法绑定 UDP 端口 {} (可能被另一实例占用): {}。将仅使用 mDNS 发现。", UDP_DISCOVERY_PORT, e);
            return Ok(shutdown);
        }
    };

    let _ = socket.set_broadcast(true);

    tauri::async_runtime::spawn(async move {
        let mut buf = [0u8; 2048];
        loop {
            tokio::select! {
                _ = shutdown_clone.notified() => {
                    debug!("UDP 监听器收到关闭信号");
                    break;
                }
                result = socket.recv_from(&mut buf) => {
                    match result {
                        Ok((len, src)) => {
                            let data = &buf[..len];
                            match serde_json::from_slice::<DiscoveryMessage>(data) {
                                Ok(msg) if msg.msg_type == "lingchat-discover" => {
                                    // 收到探测，回复自身信息
                                    debug!("收到 UDP 探测来自 {}", src);
                                    let _ = socket.send_to(&response_data, src).await;
                                }
                                _ => {}
                            }
                        }
                        Err(e) => {
                            debug!("UDP 接收错误: {}", e);
                        }
                    }
                }
            }
        }
    });

    info!("UDP 监听器已启动在端口 {}", UDP_DISCOVERY_PORT);
    Ok(shutdown)
}

// ─── 发现对等设备 ────────────────────────────────────────────

/// 扫描局域网设备，排除自身 instance。
///
/// 复用 `Announcer` 持有的 daemon（不创建新的），UDP 结果排在 mDNS 之前。
/// 去重时后出现的覆盖先出现的。
pub async fn discover_peers(
    _app: &AppHandle,
    daemon: &ServiceDaemon,
    my_instance_id: &str,
) -> Result<Vec<PeerInfo>, String> {
    let (mdns_peers, udp_peers) = tokio::join!(
        browse_mdns(daemon, my_instance_id),
        send_udp_broadcast(my_instance_id),
    );

    // UDP 优先排在前面（源 IP 是真实网络路径，比 mDNS 缓存可靠）
    let mut all: Vec<PeerInfo> = Vec::new();
    if let Ok(p) = udp_peers {
        all.extend(p);
    }
    if let Ok(p) = mdns_peers {
        all.extend(p);
    }

    // 去重：后出现的覆盖先出现的（HashMap::insert 的语义）
    let mut seen: std::collections::HashMap<String, PeerInfo> = std::collections::HashMap::new();
    for p in all {
        if p.instance_id == my_instance_id {
            continue;
        }
        info!("发现设备: {} @ {}:{}", p.device_name, p.host, p.port);
        seen.insert(p.instance_id.clone(), p);
    }

    Ok(seen.into_values().collect())
}

// ─── mDNS 浏览 ───────────────────────────────────────────────

async fn browse_mdns(
    daemon: &ServiceDaemon,
    my_instance_id: &str,
) -> Result<Vec<PeerInfo>, String> {
    let service_type = MDNS_SERVICE_TYPE.to_string();
    let my_iid = my_instance_id.to_string();
    let d = daemon.clone();

    tokio::task::spawn_blocking(move || {
        let receiver = d
            .browse(&service_type)
            .map_err(|e| format!("mDNS 浏览失败: {e}"))?;

        let mut found = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(4);

        while std::time::Instant::now() < deadline {
            match receiver.recv_timeout(Duration::from_millis(400)) {
                Ok(event) => {
                    if let mdns_sd::ServiceEvent::ServiceResolved(info) = event {
                        let txt = info.get_properties();

                        let instance_id = txt
                            .get_property_val_str(TXT_INSTANCE_ID)
                            .unwrap_or_default()
                            .to_string();

                        if instance_id.is_empty() || instance_id == my_iid {
                            continue;
                        }

                        let device_id = txt
                            .get_property_val_str(TXT_DEVICE_ID)
                            .unwrap_or_default()
                            .to_string();
                        let device_name = txt
                            .get_property_val_str(TXT_DEVICE_NAME)
                            .unwrap_or("Unknown")
                            .to_string();
                        let data_version: u64 = txt
                            .get_property_val_str(TXT_DATA_VERSION)
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(0);
                        let file_count: u64 = txt
                            .get_property_val_str(TXT_FILE_COUNT)
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(0);
                        let port = info.get_port();
                        let host = info
                            .get_addresses()
                            .iter()
                            .next()
                            .map(|a| a.to_string())
                            .unwrap_or_default();

                        if !host.is_empty() {
                            found.push(PeerInfo {
                                device_id,
                                instance_id,
                                device_name,
                                host,
                                port,
                                data_version,
                                file_count,
                            });
                        }
                    }
                }
                Err(_) => break,
            }
        }

        // ─── 干净关闭浏览 ────────────────────────────
        // 防止 daemon 线程在 receiver drop 后继续尝试
        // 重传 SearchStarted 到已关闭的 channel。
        match d.stop_browse(&service_type) {
            Ok(()) => debug!("已发送 stop_browse"),
            Err(e) => warn!("发送 stop_browse 失败: {e}"),
        }
        // 排空 channel 中的残留事件（包括 SearchStopped）
        loop {
            match receiver.recv_timeout(Duration::from_millis(500)) {
                Ok(event) => {
                    if matches!(event, mdns_sd::ServiceEvent::SearchStopped(_)) {
                        debug!("mDNS 浏览已干净关闭");
                        break;
                    }
                }
                Err(_) => break,
            }
        }

        Ok(found)
    })
    .await
    .map_err(|e| format!("mDNS 浏览任务失败: {e}"))?
}

// ─── UDP 广播探测 ─────────────────────────────────────────────

async fn send_udp_broadcast(my_instance_id: &str) -> Result<Vec<PeerInfo>, String> {
    let socket = UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(|e| format!("UDP 绑定失败: {e}"))?;
    socket
        .set_broadcast(true)
        .map_err(|e| format!("设置广播失败: {e}"))?;

    let discover_msg = DiscoveryMessage {
        msg_type: "lingchat-discover".to_string(),
        device_id: String::new(),
        instance_id: my_instance_id.to_string(),
        device_name: String::new(),
        host: String::new(),
        port: 0,
        data_version: 0,
        file_count: 0,
    };

    let json =
        serde_json::to_vec(&discover_msg).map_err(|e| format!("序列化失败: {e}"))?;

    let broadcast_addr = format!("255.255.255.255:{}", UDP_DISCOVERY_PORT);
    socket
        .send_to(&json, &broadcast_addr)
        .await
        .map_err(|e| format!("UDP 广播发送失败: {e}"))?;

    debug!("已发送 UDP 广播探测");

    // 等待响应
    let mut buf = [0u8; 2048];
    let mut peers = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);

    loop {
        match tokio::time::timeout_at(deadline, socket.recv_from(&mut buf)).await {
            Ok(Ok((len, src))) => {
                if let Ok(msg) = serde_json::from_slice::<DiscoveryMessage>(&buf[..len]) {
                    if msg.msg_type == "lingchat-response"
                        && msg.instance_id != my_instance_id
                        && !msg.host.is_empty()
                    {
                        peers.push(PeerInfo {
                            device_id: msg.device_id,
                            instance_id: msg.instance_id,
                            device_name: msg.device_name,
                            host: src.ip().to_string(),
                            port: msg.port,
                            data_version: msg.data_version,
                            file_count: msg.file_count,
                        });
                    }
                }
            }
            Ok(Err(e)) => {
                debug!("UDP 接收错误: {}", e);
                break;
            }
            Err(_) => break, // 超时
        }
    }

    Ok(peers)
}

// ─── 获取本机局域网 IP ───────────────────────────────────────

/// 获取本机可用于 LAN 同步的 IPv4 地址列表。
///
/// - 排除虚拟网卡 / 特殊网段（Hyper-V、WSL、VPN、Docker 等）
/// - 优先返回常见局域网地址（192.168.x.x / 10.x.x.x / 172.16-31.x.x）
/// - 其余非排除地址排在末尾
fn get_local_ips() -> Result<Vec<String>, String> {
    let ifaces =
        if_addrs::get_if_addrs().map_err(|e| format!("获取网卡列表失败: {e}"))?;

    let mut lan: Vec<String> = Vec::new(); // 标准局域网地址
    let mut other: Vec<String> = Vec::new(); // 其余非排除地址

    for iface in ifaces {
        if iface.is_loopback() {
            continue;
        }
        let std::net::IpAddr::V4(ip) = iface.ip() else {
            continue;
        };
        let octets = ip.octets();

        match octets {
            // 排除不可路由 / 虚拟网段
            [0, ..] => continue,                    // 0.0.0.0/8 当前网络
            [127, ..] => continue,                  // loopback（已过滤，防御性保留）
            [169, 254, ..] => continue,             // APIPA 链路本地
            [100, 64..=127, ..] => continue,        // 100.64.0.0/10 CGNAT
            [198, 18..=19, ..] => continue,         // 198.18.0.0/15 RFC 2544 基准测试
            [224..=239, ..] => continue,            // 组播
            [240..=255, ..] => continue,            // E 类 / 广播

            // 优先：标准私有局域网
            [192, 168, ..] => lan.push(ip.to_string()),
            [10, ..] => lan.push(ip.to_string()),
            [172, 16..=31, ..] => lan.push(ip.to_string()),

            // 其余公网 / 未知地址放后面
            _ => other.push(ip.to_string()),
        }
    }

    lan.append(&mut other);

    if lan.is_empty() {
        lan.push("127.0.0.1".to_string());
    }

    Ok(lan)
}
