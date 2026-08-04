use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

const MDNS_PORT: u16 = 5353;
const MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 251);
const SERVICE_TYPE: &str = "_siegu._tcp.local.";
const MDNS_TTL: u32 = 120;

#[derive(Debug, Clone, serde::Serialize)]
pub struct DiscoveredHost {
    pub name: String,
    pub ip: String,
    pub port: u16,
    #[serde(default)]
    pub room_id: String,
}

#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    Added(DiscoveredHost),
    Removed(String),
}

struct Service {
    instance: String,
    hostname: String,
    port: u16,
    txt: Vec<(String, String)>,
    ip: IpAddr,
}

enum ControlMsg {
    Register(Service),
    Unregister(String),
    Browse(String, mpsc::Sender<Vec<DiscoveredHost>>),
    Shutdown,
}

pub struct DaemonHandle {
    control: mpsc::Sender<ControlMsg>,
    shutdown: Arc<AtomicBool>,
}

impl DaemonHandle {
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = self.control.send(ControlMsg::Shutdown);
    }
}

impl Clone for DaemonHandle {
    fn clone(&self) -> Self {
        Self {
            control: self.control.clone(),
            shutdown: Arc::clone(&self.shutdown),
        }
    }
}

pub fn create_daemon() -> Result<DaemonHandle, Box<dyn std::error::Error>> {
    let (tx, rx) = mpsc::channel();
    let shutdown = Arc::new(AtomicBool::new(false));
    let handle = DaemonHandle {
        control: tx,
        shutdown: Arc::clone(&shutdown),
    };

    thread::spawn(move || {
        let socket = bind_multicast_socket();
        let socket = match socket {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to bind mDNS socket: {e}");
                return;
            }
        };
        socket
            .set_read_timeout(Some(Duration::from_millis(200)))
            .ok();

        let mut services: Vec<Service> = Vec::new();
        let mut responders: Vec<mpsc::Sender<Vec<DiscoveredHost>>> = Vec::new();
        // mDNS packets are UDP multicast and can be dropped, so a single PTR
        // query is not enough (RFC 6762 §5.2). Re-send while anyone browses.
        let mut last_query = Instant::now();

        let mut buf = [0u8; 1500];

        loop {
            if shutdown.load(Ordering::SeqCst) {
                break;
            }

            while let Ok(msg) = rx.try_recv() {
                match msg {
                    ControlMsg::Register(svc) => {
                        services.push(svc);
                    }
                    ControlMsg::Unregister(instance) => {
                        if let Some(pos) = services.iter().position(|s| s.instance == instance) {
                            let svc = services.remove(pos);
                            let goodbye = build_goodbye(&svc);
                            let _ = socket.send_to(
                                &goodbye,
                                SocketAddr::new(MULTICAST_ADDR.into(), MDNS_PORT),
                            );
                        }
                    }
                    ControlMsg::Browse(ty, sender) => {
                        let query = build_ptr_query(&ty);
                        socket
                            .send_to(&query, SocketAddr::new(MULTICAST_ADDR.into(), MDNS_PORT))
                            .ok();
                        last_query = Instant::now();
                        // A daemon can discover services it itself advertises
                        // without waiting for multicast loopback: on macOS the
                        // looped-back packet may be delivered to mDNSResponder's
                        // SO_REUSEPORT socket instead of ours, making
                        // self-discovery racy. Answer in-process (RFC 6762
                        // expects a host to see its own instances). No-op for
                        // production browse-only daemons, which have no services.
                        if !services.is_empty() {
                            let local: Vec<DiscoveredHost> = services
                                .iter()
                                .map(|s| DiscoveredHost {
                                    name: s.instance.clone(),
                                    ip: s.ip.to_string(),
                                    port: s.port,
                                    room_id: String::new(),
                                })
                                .collect();
                            let _ = sender.send(local);
                        }
                        responders.push(sender);
                    }
                    ControlMsg::Shutdown => return,
                }
            }

            if !responders.is_empty() && last_query.elapsed() >= Duration::from_secs(1) {
                let query = build_ptr_query(SERVICE_TYPE);
                if socket
                    .send_to(&query, SocketAddr::new(MULTICAST_ADDR.into(), MDNS_PORT))
                    .is_ok()
                {
                    last_query = Instant::now();
                }
            }

            if let Ok(sz) = socket.recv(&mut buf) {
                let packet = &buf[..sz];
                if is_query(packet) && has_siegu_question(packet) {
                    if let Some(resp) = build_response(packet, &services) {
                        socket
                            .send_to(&resp, SocketAddr::new(MULTICAST_ADDR.into(), MDNS_PORT))
                            .ok();
                    }
                }
                if is_response(packet) && has_siegu_answer(packet) {
                    let hosts = parse_hosts_from_response(packet);
                    if !hosts.is_empty() {
                        for sender in &responders {
                            let _ = sender.send(hosts.clone());
                        }
                    }
                }
            }
        }
    });

    Ok(handle)
}

fn bind_multicast_socket() -> Result<UdpSocket, Box<dyn std::error::Error>> {
    let sock = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )?;
    sock.set_reuse_address(true)?;
    // macOS/BSD require SO_REUSEPORT to bind UDP 5353 alongside
    // mDNSResponder, which already owns the port. Unix-only: socket2 does not
    // expose set_reuse_port on Windows (SO_EXCLUSIVEADDRUSE semantics differ).
    #[cfg(unix)]
    sock.set_reuse_port(true)?;
    if let Err(e) = sock.set_read_timeout(Some(Duration::from_millis(200))) {
        eprintln!("set_read_timeout: {e}");
    }
    let addr: SocketAddr = (Ipv4Addr::UNSPECIFIED, MDNS_PORT).into();
    sock.bind(&addr.into())?;
    sock.set_multicast_loop_v4(true)?;
    sock.join_multicast_v4(&MULTICAST_ADDR, &Ipv4Addr::UNSPECIFIED)?;
    Ok(UdpSocket::from(sock))
}

/// Register a service for LAN discovery. Note: no room credential is published
/// in the TXT records — the room key must be derived from the passphrase by
/// the joiner, never broadcast on the wire.
pub fn register_service(
    daemon: &DaemonHandle,
    hostname: &str,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let ip = local_ip()?;
    let svc = Service {
        instance: hostname.to_string(),
        hostname: format!("{}.local.", hostname.trim_end_matches('.')),
        port,
        txt: vec![(
            "version".to_string(),
            crate::mesh::PROTOCOL_VERSION.to_string(),
        )],
        ip,
    };
    daemon.control.send(ControlMsg::Register(svc))?;
    Ok(())
}

pub fn unregister_service(daemon: &DaemonHandle, hostname: &str) {
    let _ = daemon
        .control
        .send(ControlMsg::Unregister(hostname.to_string()));
}

pub fn discover_hosts(
    daemon: &DaemonHandle,
    timeout_secs: u64,
) -> Result<Vec<DiscoveredHost>, Box<dyn std::error::Error>> {
    let (tx, rx) = mpsc::channel();
    daemon
        .control
        .send(ControlMsg::Browse(SERVICE_TYPE.to_string(), tx))?;

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let mut hosts = HashMap::new();

    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(list) => {
                for h in list {
                    let key = format!("{}:{}", h.ip, h.port);
                    hosts.entry(key).or_insert(h);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        }
    }

    Ok(hosts.into_values().collect())
}

pub fn watch_hosts(
    daemon: &DaemonHandle,
) -> Result<mpsc::Receiver<DiscoveryEvent>, Box<dyn std::error::Error>> {
    let (tx, rx) = mpsc::channel();
    let control = daemon.control.clone();

    thread::spawn(move || {
        let (resp_tx, resp_rx) = mpsc::channel();
        if control
            .send(ControlMsg::Browse(SERVICE_TYPE.to_string(), resp_tx))
            .is_err()
        {
            return;
        }
        loop {
            match resp_rx.recv_timeout(Duration::from_secs(1)) {
                Ok(list) => {
                    for h in list {
                        if tx.send(DiscoveryEvent::Added(h)).is_err() {
                            return;
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => return,
            }
        }
    });

    Ok(rx)
}

fn local_ip() -> Result<IpAddr, Box<dyn std::error::Error>> {
    let s = UdpSocket::bind("0.0.0.0:0")?;
    s.connect("8.8.8.8:53")?;
    Ok(s.local_addr()?.ip())
}

// --- DNS wire format helpers ---

fn is_query(buf: &[u8]) -> bool {
    buf.len() >= 12 && (buf[2] & 0x80) == 0
}

fn is_response(buf: &[u8]) -> bool {
    buf.len() >= 12 && (buf[2] & 0x80) != 0
}

fn has_siegu_question(buf: &[u8]) -> bool {
    name_contains(buf, 12, b"_siegu")
}

fn has_siegu_answer(buf: &[u8]) -> bool {
    buf.windows(6).any(|w| w == b"_siegu")
}

fn name_contains(buf: &[u8], offset: usize, target: &[u8]) -> bool {
    let mut pos = offset;
    while pos < buf.len() {
        let len = buf[pos] as usize;
        if len & 0xC0 == 0xC0 {
            break;
        }
        if len == 0 {
            break;
        }
        pos += 1;
        if pos + len > buf.len() {
            break;
        }
        if &buf[pos..pos + len] == target {
            return true;
        }
        pos += len;
    }
    false
}

fn skip_name(buf: &[u8], offset: usize) -> usize {
    let mut pos = offset;
    while pos < buf.len() {
        let len = buf[pos];
        if len & 0xC0 == 0xC0 {
            return pos + 2;
        }
        if len == 0 {
            return pos + 1;
        }
        pos += 1 + len as usize;
    }
    pos
}

fn encode_name(buf: &mut Vec<u8>, name: &str) {
    for label in name.trim_end_matches('.').split('.') {
        if label.is_empty() {
            continue;
        }
        buf.push(label.len() as u8);
        buf.extend_from_slice(label.as_bytes());
    }
    buf.push(0);
}

fn build_ptr_query(service_type: &str) -> Vec<u8> {
    let mut buf = Vec::with_capacity(64);
    buf.extend_from_slice(&[0u8; 2]); // ID = 0
    buf.extend_from_slice(&[0x00, 0x00]); // flags = query
    buf.extend_from_slice(&[0x00, 0x01]); // QDCOUNT = 1
    buf.extend_from_slice(&[0x00, 0x00]); // ANCOUNT = 0
    buf.extend_from_slice(&[0x00, 0x00]); // NSCOUNT = 0
    buf.extend_from_slice(&[0x00, 0x00]); // ARCOUNT = 0
    encode_name(&mut buf, service_type);
    buf.extend_from_slice(&[0x00, 0x0C]); // QTYPE = PTR
    buf.extend_from_slice(&[0x00, 0x01]); // QCLASS = IN
    buf
}

fn build_response(query: &[u8], services: &[Service]) -> Option<Vec<u8>> {
    if services.is_empty() {
        return None;
    }

    let question_end = skip_name(query, 12);
    let qtype = u16::from_be_bytes([query[question_end], query[question_end + 1]]);
    if qtype != 12 && qtype != 255 {
        return None;
    }

    let svc = &services[0];
    let mut buf = Vec::with_capacity(512);

    // Header: copy ID, set response flags
    buf.extend_from_slice(&query[..2]); // ID
    buf.extend_from_slice(&[0x84, 0x00]); // flags = response + authoritative
    buf.extend_from_slice(&[0x00, 0x01]); // QDCOUNT = 1 (echo question)
    buf.extend_from_slice(&[0x00, 0x01]); // ANCOUNT = 1 (PTR)
    buf.extend_from_slice(&[0x00, 0x00]); // NSCOUNT = 0
    buf.extend_from_slice(&[0x00, 0x03]); // ARCOUNT = 3 (SRV, TXT, A)

    // Echo the question
    let qname_start = buf.len();
    let question = &query[12..question_end + 4];
    buf.extend_from_slice(question);

    // PTR answer: name=_siegu._tcp.local (use pointer to qname), type=PTR, class=IN (no cache flush for shared PTR records), TTL=120
    let ptr_offset = qname_start;
    buf.push(0xC0);
    buf.push(ptr_offset as u8);
    buf.extend_from_slice(&[0x00, 0x0C]); // TYPE = PTR
    buf.extend_from_slice(&[0x00, 0x01]); // CLASS = IN (shared record - no cache flush)
    buf.extend_from_slice(&MDNS_TTL.to_be_bytes());
    // RDDATA: instance name
    let rdlen_pos = buf.len();
    buf.extend_from_slice(&[0u8; 2]); // placeholder
    let data_start = buf.len();
    encode_name(&mut buf, &format!("{}.{}", svc.instance, SERVICE_TYPE));
    let rdlen = (buf.len() - data_start) as u16;
    buf[rdlen_pos..rdlen_pos + 2].copy_from_slice(&rdlen.to_be_bytes());

    // SRV additional: name=instance._siegu._tcp.local (use pointer to data_start), type=SRV, class=IN, TTL=120
    buf.push(0xC0);
    buf.push(data_start as u8);
    buf.extend_from_slice(&[0x00, 0x21]); // TYPE = SRV
    buf.extend_from_slice(&[0x80, 0x01]); // CLASS = IN + cache flush
    buf.extend_from_slice(&MDNS_TTL.to_be_bytes());
    let rdlen_pos2 = buf.len();
    buf.extend_from_slice(&[0u8; 2]);
    let data_start2 = buf.len();
    buf.extend_from_slice(&[0x00, 0x00]); // priority = 0
    buf.extend_from_slice(&[0x00, 0x00]); // weight = 0
    buf.extend_from_slice(&svc.port.to_be_bytes()); // port
    encode_name(&mut buf, &svc.hostname); // target hostname
    let rdlen2 = (buf.len() - data_start2) as u16;
    buf[rdlen_pos2..rdlen_pos2 + 2].copy_from_slice(&rdlen2.to_be_bytes());

    // TXT additional: name=instance._siegu._tcp.local (pointer to data_start)
    buf.push(0xC0);
    buf.push(data_start as u8);
    buf.extend_from_slice(&[0x00, 0x10]); // TYPE = TXT
    buf.extend_from_slice(&[0x80, 0x01]); // CLASS = IN + cache flush
    buf.extend_from_slice(&MDNS_TTL.to_be_bytes());
    let rdlen_pos3 = buf.len();
    buf.extend_from_slice(&[0u8; 2]);
    let data_start3 = buf.len();
    for (key, value) in &svc.txt {
        let entry = format!("{key}={value}");
        buf.push(entry.len() as u8);
        buf.extend_from_slice(entry.as_bytes());
    }
    let rdlen3 = (buf.len() - data_start3) as u16;
    buf[rdlen_pos3..rdlen_pos3 + 2].copy_from_slice(&rdlen3.to_be_bytes());

    // A additional: name=hostname.local.
    encode_name(&mut buf, &svc.hostname);
    buf.extend_from_slice(&[0x00, 0x01]); // TYPE = A
    buf.extend_from_slice(&[0x80, 0x01]); // CLASS = IN + cache flush
    buf.extend_from_slice(&MDNS_TTL.to_be_bytes());
    buf.extend_from_slice(&[0x00, 0x04]); // RDLENGTH = 4
    match svc.ip {
        IpAddr::V4(ip) => buf.extend_from_slice(&ip.octets()),
        IpAddr::V6(_) => {} // skip AAAA for now
    }

    Some(buf)
}

/// Unsolicited mDNS goodbye response for a service being unregistered. All
/// records are sent with TTL 0 so caches (e.g. Android NsdManager) drop the
/// stale instance instead of resolving a dead/orphaned port.
fn build_goodbye(svc: &Service) -> Vec<u8> {
    let mut buf = Vec::with_capacity(256);
    buf.extend_from_slice(&[0x00, 0x00]); // ID = 0
    buf.extend_from_slice(&[0x84, 0x00]); // flags = response
    buf.extend_from_slice(&[0x00, 0x00]); // QDCOUNT = 0
    buf.extend_from_slice(&[0x00, 0x01]); // ANCOUNT = 1 (PTR)
    buf.extend_from_slice(&[0x00, 0x00]); // NSCOUNT = 0
    buf.extend_from_slice(&[0x00, 0x03]); // ARCOUNT = 3 (SRV, TXT, A)

    // PTR: name=_siegu._tcp.local., target=instance._siegu._tcp.local., TTL=0
    encode_name(&mut buf, SERVICE_TYPE);
    buf.extend_from_slice(&[0x00, 0x0C]); // TYPE = PTR
    buf.extend_from_slice(&[0x00, 0x01]); // CLASS = IN
    buf.extend_from_slice(&[0u8; 4]); // TTL = 0
    let rdlen_pos = buf.len();
    buf.extend_from_slice(&[0u8; 2]);
    let data_start = buf.len();
    encode_name(&mut buf, &format!("{}.{}", svc.instance, SERVICE_TYPE));
    let rdlen = (buf.len() - data_start) as u16;
    buf[rdlen_pos..rdlen_pos + 2].copy_from_slice(&rdlen.to_be_bytes());

    // SRV: name=instance._siegu._tcp.local., TTL=0
    buf.push(0xC0);
    buf.push(data_start as u8);
    buf.extend_from_slice(&[0x00, 0x21]); // TYPE = SRV
    buf.extend_from_slice(&[0x80, 0x01]); // CLASS = IN + cache flush
    buf.extend_from_slice(&[0u8; 4]); // TTL = 0
    let rdlen_pos2 = buf.len();
    buf.extend_from_slice(&[0u8; 2]);
    let data_start2 = buf.len();
    buf.extend_from_slice(&[0x00, 0x00]); // priority = 0
    buf.extend_from_slice(&[0x00, 0x00]); // weight = 0
    buf.extend_from_slice(&svc.port.to_be_bytes()); // port
    encode_name(&mut buf, &svc.hostname); // target hostname
    let rdlen2 = (buf.len() - data_start2) as u16;
    buf[rdlen_pos2..rdlen_pos2 + 2].copy_from_slice(&rdlen2.to_be_bytes());

    // TXT: name=instance._siegu._tcp.local., TTL=0
    buf.push(0xC0);
    buf.push(data_start as u8);
    buf.extend_from_slice(&[0x00, 0x10]); // TYPE = TXT
    buf.extend_from_slice(&[0x80, 0x01]); // CLASS = IN + cache flush
    buf.extend_from_slice(&[0u8; 4]); // TTL = 0
    let rdlen_pos3 = buf.len();
    buf.extend_from_slice(&[0u8; 2]);
    let data_start3 = buf.len();
    for (key, value) in &svc.txt {
        let entry = format!("{key}={value}");
        buf.push(entry.len() as u8);
        buf.extend_from_slice(entry.as_bytes());
    }
    let rdlen3 = (buf.len() - data_start3) as u16;
    buf[rdlen_pos3..rdlen_pos3 + 2].copy_from_slice(&rdlen3.to_be_bytes());

    // A: name=hostname.local., TTL=0
    encode_name(&mut buf, &svc.hostname);
    buf.extend_from_slice(&[0x00, 0x01]); // TYPE = A
    buf.extend_from_slice(&[0x80, 0x01]); // CLASS = IN + cache flush
    buf.extend_from_slice(&[0u8; 4]); // TTL = 0
    buf.extend_from_slice(&[0x00, 0x04]); // RDLENGTH = 4
    if let IpAddr::V4(ip) = svc.ip {
        buf.extend_from_slice(&ip.octets());
    }

    buf
}

fn parse_hosts_from_response(buf: &[u8]) -> Vec<DiscoveredHost> {
    let mut hosts = Vec::new();
    if buf.len() < 12 {
        return hosts;
    }

    let ancount = u16::from_be_bytes([buf[6], buf[7]]);
    let arcount = u16::from_be_bytes([buf[10], buf[11]]);

    let mut pos = 12;
    // skip question
    pos = skip_name(buf, pos);
    if pos + 4 > buf.len() {
        return hosts;
    }
    pos += 4; // skip qtype + qclass

    let mut srv_records: Vec<(String, u16, String)> = Vec::new();
    let mut a_records: HashMap<String, String> = HashMap::new();
    let mut ptr_records: Vec<String> = Vec::new();
    let mut txt_records: HashMap<String, String> = HashMap::new();

    // Parse answers
    for _ in 0..ancount + arcount {
        let name = parse_name(buf, pos);
        pos = skip_name_compressed(buf, pos);
        if pos + 10 > buf.len() {
            break;
        }

        let rtype = u16::from_be_bytes([buf[pos], buf[pos + 1]]);
        let _rclass = u16::from_be_bytes([buf[pos + 2], buf[pos + 3]]);
        let _ttl = u32::from_be_bytes([buf[pos + 4], buf[pos + 5], buf[pos + 6], buf[pos + 7]]);
        let rdlen = u16::from_be_bytes([buf[pos + 8], buf[pos + 9]]) as usize;
        pos += 10;

        if pos + rdlen > buf.len() {
            break;
        }

        match rtype {
            1 => {
                // A
                if rdlen == 4 {
                    let ip = format!(
                        "{}.{}.{}.{}",
                        buf[pos],
                        buf[pos + 1],
                        buf[pos + 2],
                        buf[pos + 3]
                    );
                    a_records.insert(name.clone(), ip);
                }
            }
            12 => {
                // PTR
                let target = parse_name(buf, pos);
                ptr_records.push(target);
            }
            33 => {
                // SRV
                if rdlen >= 6 {
                    let _priority = u16::from_be_bytes([buf[pos], buf[pos + 1]]);
                    let _weight = u16::from_be_bytes([buf[pos + 2], buf[pos + 3]]);
                    let port = u16::from_be_bytes([buf[pos + 4], buf[pos + 5]]);
                    let target = parse_name(buf, pos + 6);
                    srv_records.push((name.clone(), port, target));
                }
            }
            16 => {
                // TXT: sequence of <len><bytes> strings, e.g. "room=<id>"
                let mut room = String::new();
                let mut off = pos;
                let end = pos + rdlen;
                while off < end {
                    let l = buf[off] as usize;
                    off += 1;
                    if off + l > end {
                        break;
                    }
                    let s = String::from_utf8_lossy(&buf[off..off + l]).to_string();
                    if let Some(v) = s.strip_prefix("room=") {
                        room = v.to_string();
                    }
                    off += l;
                }
                txt_records.insert(name.trim_end_matches('.').to_string(), room);
            }
            _ => {}
        }

        pos += rdlen;
    }

    for ptr in &ptr_records {
        if let Some(srv) = srv_records
            .iter()
            .find(|(n, _, _)| ptr.contains(n) || n.contains(ptr))
        {
            let hostname = srv.2.trim_end_matches('.').to_string();
            let ip = a_records.get(&hostname).or_else(|| {
                // try matching with or without trailing dot
                let with_dot = format!("{hostname}.");
                a_records
                    .keys()
                    .find(|k| *k == &with_dot)
                    .and_then(|k| a_records.get(k))
            });

            if let Some(ip) = ip {
                hosts.push(DiscoveredHost {
                    name: srv.0.trim_end_matches('.').to_string(),
                    ip: ip.clone(),
                    port: srv.1,
                    room_id: txt_records
                        .get(srv.0.trim_end_matches('.'))
                        .cloned()
                        .unwrap_or_default(),
                });
            }
        }
    }

    hosts
}

fn parse_name(buf: &[u8], offset: usize) -> String {
    let mut labels = Vec::new();
    let mut pos = offset;

    loop {
        if pos >= buf.len() {
            break;
        }
        let len = buf[pos] as usize;
        if len & 0xC0 == 0xC0 {
            if pos + 1 >= buf.len() {
                break;
            }
            let ptr = ((len & 0x3F) << 8) | buf[pos + 1] as usize;
            let pointed_name = parse_name(buf, ptr);
            labels.push(pointed_name);
            break;
        }
        if len == 0 {
            break;
        }
        pos += 1;
        if pos + len > buf.len() {
            break;
        }
        labels.push(String::from_utf8_lossy(&buf[pos..pos + len]).to_string());
        pos += len;
    }

    labels.join(".")
}

fn skip_name_compressed(buf: &[u8], offset: usize) -> usize {
    let mut pos = offset;
    loop {
        if pos >= buf.len() {
            return pos;
        }
        let len = buf[pos];
        if len & 0xC0 == 0xC0 {
            return pos + 2;
        }
        if len == 0 {
            return pos + 1;
        }
        pos += 1 + len as usize;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broadcast_response_does_not_leak_room_id() {
        let svc = Service {
            instance: "testhost".to_string(),
            hostname: "testhost.local.".to_string(),
            port: 9999,
            txt: vec![("version".to_string(), "1".to_string())],
            ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        };

        let query = build_ptr_query(SERVICE_TYPE);
        let resp = build_response(&query, &[svc]).expect("response");
        let s = String::from_utf8_lossy(&resp);

        assert!(
            s.contains("version=1"),
            "version TXT should still be advertised"
        );
        assert!(
            !s.contains("room="),
            "room credential must never be broadcast via mDNS"
        );
    }

    #[test]
    fn parse_hosts_without_txt_room_still_resolves() {
        // A response with no room= TXT (new behaviour) must still yield a
        // DiscoveredHost, with an empty room_id.
        let svc = Service {
            instance: "testhost".to_string(),
            hostname: "testhost.local.".to_string(),
            port: 9999,
            txt: vec![("version".to_string(), "1".to_string())],
            ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        };

        let query = build_ptr_query(SERVICE_TYPE);
        let buf = build_response(&query, &[svc]).expect("response");

        let hosts = parse_hosts_from_response(&buf);
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].name, "testhost._siegu._tcp.local");
        assert_eq!(hosts[0].port, 9999);
        assert_eq!(hosts[0].ip, "127.0.0.1");
        assert_eq!(hosts[0].room_id, "");
    }
}
