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
                        services.retain(|s| s.instance != instance);
                    }
                    ControlMsg::Browse(ty, sender) => {
                        let query = build_ptr_query(&ty);
                        socket
                            .send_to(&query, SocketAddr::new(MULTICAST_ADDR.into(), MDNS_PORT))
                            .ok();
                        responders.push(sender);
                    }
                    ControlMsg::Shutdown => return,
                }
            }

            if let Ok((sz, src)) = socket.recv_from(&mut buf) {
                let packet = &buf[..sz];
                if is_query(packet) && has_siegu_question(packet) {
                    if let Some(resp) = build_response(packet, &services) {
                        // Reply to the querying address (covers both multicast PTR
                        // queries and the unicast SRV/TXT/A queries that Android's
                        // NsdManager sends during resolveService()).
                        socket.send_to(&resp, src).ok();
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
    if let Err(e) = sock.set_read_timeout(Some(Duration::from_millis(200))) {
        eprintln!("set_read_timeout: {e}");
    }
    let addr: SocketAddr = (Ipv4Addr::UNSPECIFIED, MDNS_PORT).into();
    sock.bind(&addr.into())?;
    sock.set_multicast_loop_v4(true)?;
    sock.join_multicast_v4(&MULTICAST_ADDR, &Ipv4Addr::UNSPECIFIED)?;
    Ok(UdpSocket::from(sock))
}

pub fn register_service(
    daemon: &DaemonHandle,
    hostname: &str,
    port: u16,
    room_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let ip = local_ip()?;
    let svc = Service {
        instance: hostname.to_string(),
        hostname: format!("{}.local.", hostname.trim_end_matches('.')),
        port,
        txt: vec![
            ("room".to_string(), room_id.to_string()),
            (
                "version".to_string(),
                crate::mesh::PROTOCOL_VERSION.to_string(),
            ),
        ],
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
    name_contains(buf, 12, b"siegu")
}

fn has_siegu_answer(buf: &[u8]) -> bool {
    buf.windows(6).any(|w| w == b"siegu")
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
    if question_end + 4 > query.len() {
        return None;
    }
    let qtype = u16::from_be_bytes([query[question_end], query[question_end + 1]]);
    // 1 = A, 12 = PTR, 16 = TXT, 33 = SRV, 255 = ANY. Android's NsdManager asks
    // for SRV/TXT/A during resolveService(); answer all of them or it times out.
    if !matches!(qtype, 1 | 12 | 16 | 33 | 255) {
        return None;
    }

    let svc = &services[0];
    let service_type = SERVICE_TYPE;
    let instance_name = format!("{}.{}", svc.instance, service_type);
    let hostname = svc.hostname.clone();

    let mut a_rdata = Vec::with_capacity(4);
    if let IpAddr::V4(ip) = svc.ip {
        a_rdata.extend_from_slice(&ip.octets());
    }

    // (name, rtype, cache_flush, rdata)
    let ptr = (
        service_type.to_string(),
        12u16,
        false,
        encode_name_bytes(&instance_name),
    );
    let srv = (
        instance_name.clone(),
        33u16,
        true,
        srv_rdata(svc.port, &hostname),
    );
    let txt = (instance_name, 16u16, true, txt_rdata(&svc.txt));
    let a = (hostname, 1u16, true, a_rdata);

    let (answers, additionals): (Vec<(String, u16, bool, Vec<u8>)>, _) = match qtype {
        1 => (vec![a], Vec::new()),
        12 => (vec![ptr], vec![srv, txt, a]),
        16 => (vec![txt], vec![srv, a]),
        33 => (vec![srv], vec![txt, a]),
        _ => (vec![ptr, srv, txt, a], Vec::new()),
    };

    let mut buf = Vec::with_capacity(1024);
    // Header: copy ID, set response + authoritative flags
    buf.extend_from_slice(&query[..2]);
    buf.extend_from_slice(&[0x84, 0x00]);
    buf.extend_from_slice(&[0x00, 0x01]); // QDCOUNT = 1 (echo question)
    buf.extend_from_slice(&(answers.len() as u16).to_be_bytes()); // ANCOUNT
    buf.extend_from_slice(&[0x00, 0x00]); // NSCOUNT = 0
    buf.extend_from_slice(&(additionals.len() as u16).to_be_bytes()); // ARCOUNT

    // Echo the question
    buf.extend_from_slice(&query[12..question_end + 4]);

    for (name, rtype, cache_flush, rdata) in answers.into_iter().chain(additionals) {
        write_record(&mut buf, &name, rtype, cache_flush, MDNS_TTL, &rdata);
    }

    Some(buf)
}

fn write_record(
    buf: &mut Vec<u8>,
    name: &str,
    rtype: u16,
    cache_flush: bool,
    ttl: u32,
    rdata: &[u8],
) {
    encode_name(buf, name);
    buf.extend_from_slice(&rtype.to_be_bytes());
    let rclass: u16 = if cache_flush { 0x8001 } else { 0x0001 };
    buf.extend_from_slice(&rclass.to_be_bytes());
    buf.extend_from_slice(&ttl.to_be_bytes());
    buf.extend_from_slice(&(rdata.len() as u16).to_be_bytes());
    buf.extend_from_slice(rdata);
}

fn encode_name_bytes(name: &str) -> Vec<u8> {
    let mut v = Vec::new();
    encode_name(&mut v, name);
    v
}

fn srv_rdata(port: u16, hostname: &str) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // priority = 0, weight = 0
    v.extend_from_slice(&port.to_be_bytes());
    encode_name(&mut v, hostname);
    v
}

fn txt_rdata(txt: &[(String, String)]) -> Vec<u8> {
    let mut v = Vec::new();
    for (key, value) in txt {
        let entry = format!("{key}={value}");
        v.push(entry.len() as u8);
        v.extend_from_slice(entry.as_bytes());
    }
    v
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

    fn test_service() -> Service {
        Service {
            instance: "siegu-host".to_string(),
            hostname: "siegu-host.local.".to_string(),
            port: 42951,
            txt: vec![
                ("room".to_string(), "abc123".to_string()),
                ("version".to_string(), "2".to_string()),
            ],
            ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20)),
        }
    }

    fn make_query(qname: &str, qtype: u16) -> Vec<u8> {
        let mut buf = vec![0x12, 0x34];
        buf.extend_from_slice(&[0x00, 0x00]); // flags = query
        buf.extend_from_slice(&[0x00, 0x01]); // QDCOUNT = 1
        buf.extend_from_slice(&[0x00, 0x00]);
        buf.extend_from_slice(&[0x00, 0x00]);
        buf.extend_from_slice(&[0x00, 0x00]);
        encode_name(&mut buf, qname);
        buf.extend_from_slice(&qtype.to_be_bytes());
        buf.extend_from_slice(&[0x00, 0x01]); // QCLASS = IN
        buf
    }

    #[test]
    fn test_responds_to_ptr_query_and_parses_back() {
        let services = vec![test_service()];
        let query = make_query(SERVICE_TYPE, 12);
        let resp = build_response(&query, &services).expect("PTR query must be answered");

        let hosts = parse_hosts_from_response(&resp);
        assert_eq!(hosts.len(), 1);
        let h = &hosts[0];
        assert_eq!(h.name, "siegu-host._siegu._tcp.local");
        assert_eq!(h.ip, "192.168.1.20");
        assert_eq!(h.port, 42951);
        assert_eq!(h.room_id, "abc123");
    }

    #[test]
    fn test_responds_to_srv_txt_and_a_queries() {
        let services = vec![test_service()];
        let instance = "siegu-host._siegu._tcp.local.";

        // Android NsdManager sends these during resolveService().
        assert!(
            build_response(&make_query(instance, 33), &services).is_some(),
            "SRV query must be answered"
        );
        assert!(
            build_response(&make_query(instance, 16), &services).is_some(),
            "TXT query must be answered"
        );
        assert!(
            build_response(&make_query("siegu-host.local.", 1), &services).is_some(),
            "A query must be answered"
        );
        assert!(
            build_response(&make_query(SERVICE_TYPE, 255), &services).is_some(),
            "ANY query must be answered"
        );
    }

    #[test]
    fn test_ignores_unrelated_query_types() {
        let services = vec![test_service()];
        // MX = 15, NS = 2 are not handled.
        assert!(build_response(&make_query("_siegu._tcp.local.", 15), &services).is_none());
        assert!(build_response(&make_query("_siegu._tcp.local.", 2), &services).is_none());
    }

    #[test]
    fn test_no_response_without_services() {
        let query = make_query(SERVICE_TYPE, 12);
        assert!(build_response(&query, &[]).is_none());
    }

    #[test]
    fn test_srv_response_contains_port() {
        let services = vec![test_service()];
        let query = make_query("siegu-host._siegu._tcp.local.", 33);
        let resp = build_response(&query, &services).unwrap();
        assert!(
            resp.windows(2).any(|w| w == [0xA7, 0xC7]),
            "SRV response must contain port 42951 (0xA7C7)"
        );
    }
}
