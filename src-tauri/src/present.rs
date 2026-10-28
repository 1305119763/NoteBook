use crate::media;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime};

const COLLAB_COLORS: [&str; 6] = ["#c026d3", "#ea580c", "#0284c7", "#16a34a", "#7c3aed", "#b45309"];
const COLLAB_FONTS: [&str; 6] = [
    "Georgia, serif",
    "\"Palatino Linotype\", Palatino, serif",
    "\"Trebuchet MS\", sans-serif",
    "\"Courier New\", monospace",
    "Verdana, sans-serif",
    "\"Gill Sans\", \"Gill Sans MT\", sans-serif",
];
const PEER_TTL_MS: u128 = 8000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresentPeer {
    pub ip: String,
    pub mode: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresentRequest {
    pub ip: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresentStatus {
    pub note_id: String,
    pub title: String,
    pub url: String,
    pub ip: String,
    pub port: u16,
    pub rev: u64,
    pub origin: String,
    pub html: String,
    pub max_editors: u32,
    pub peers: Vec<PresentPeer>,
    pub requests: Vec<PresentRequest>,
    pub notices: Vec<String>,
}

struct Peer {
    mode: String,
    color: String,
    last_seen: u128,
}

struct PresentDoc {
    note_id: String,
    title: String,
    html: String,
    rev: u64,
    origin: String,
    max_editors: u32,
    peers: HashMap<String, Peer>,
    requests: Vec<String>,
    editors: HashMap<String, String>,
    rejected: HashSet<String>,
    notices: HashMap<String, String>,
    owner_notices: Vec<String>,
    nickname: String,
}

struct Running {
    shutdown: Arc<AtomicBool>,
    doc: Arc<Mutex<PresentDoc>>,
    ip: String,
    port: u16,
}

fn slot() -> &'static Mutex<Option<Running>> {
    static SLOT: OnceLock<Mutex<Option<Running>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

fn lock_in<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|err| err.into_inner())
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn lan_ipv4() -> String {
    let Ok(sock) = std::net::UdpSocket::bind("0.0.0.0:0") else {
        return "127.0.0.1".into();
    };
    if sock.connect("8.8.8.8:80").is_err() {
        return "127.0.0.1".into();
    }
    match sock.local_addr() {
        Ok(SocketAddr::V4(addr)) => {
            let ip = addr.ip();
            if ip.is_loopback() {
                "127.0.0.1".into()
            } else {
                ip.to_string()
            }
        }
        _ => "127.0.0.1".into(),
    }
}

fn normalize_ip(addr: SocketAddr) -> String {
    match addr.ip() {
        IpAddr::V4(ip) => ip.to_string(),
        IpAddr::V6(ip) => ip
            .to_ipv4_mapped()
            .map(|v4| v4.to_string())
            .unwrap_or_else(|| ip.to_string()),
    }
}

fn clamp_max(max_editors: u32) -> u32 {
    max_editors.clamp(1, 12)
}

fn font_for(color: &str) -> &'static str {
    let idx = COLLAB_COLORS.iter().position(|item| *item == color).unwrap_or(0);
    COLLAB_FONTS[idx % COLLAB_FONTS.len()]
}

fn next_color(doc: &PresentDoc) -> String {
    let used: HashSet<&str> = doc.editors.values().map(|c| c.as_str()).collect();
    for color in COLLAB_COLORS {
        if !used.contains(color) {
            return color.to_string();
        }
    }
    COLLAB_COLORS[doc.editors.len() % COLLAB_COLORS.len()].to_string()
}

fn prune(doc: &mut PresentDoc) {
    let now = now_ms();
    doc.peers.retain(|_, peer| now.saturating_sub(peer.last_seen) < PEER_TTL_MS);
    doc.editors.retain(|ip, _| doc.peers.contains_key(ip));
    doc.requests
        .retain(|ip| doc.peers.contains_key(ip) && !doc.editors.contains_key(ip));
}

fn observe(doc: &mut PresentDoc, ip: &str) {
    let now = now_ms();
    let color = doc.editors.get(ip).cloned().unwrap_or_default();
    let mode = if doc.editors.contains_key(ip) { "edit" } else { "view" };
    doc.peers.insert(
        ip.to_string(),
        Peer {
            mode: mode.to_string(),
            color,
            last_seen: now,
        },
    );
    prune(doc);
    if let Some(peer) = doc.peers.get_mut(ip) {
        peer.mode = if doc.editors.contains_key(ip) {
            "edit".into()
        } else {
            "view".into()
        };
        peer.color = doc.editors.get(ip).cloned().unwrap_or_default();
        peer.last_seen = now;
    }
}

fn role_for(doc: &PresentDoc, ip: &str) -> String {
    if doc.editors.contains_key(ip) {
        return "edit".into();
    }
    if doc.rejected.contains(ip) {
        return "rejected".into();
    }
    if doc.requests.iter().any(|item| item == ip) {
        return "pending".into();
    }
    if doc.editors.len() >= doc.max_editors as usize {
        return "full".into();
    }
    "view".into()
}

fn status_of(running: &Running, doc: &mut PresentDoc) -> PresentStatus {
    let mut peers: Vec<PresentPeer> = doc
        .peers
        .iter()
        .map(|(ip, peer)| PresentPeer {
            ip: ip.clone(),
            mode: peer.mode.clone(),
            color: peer.color.clone(),
        })
        .collect();
    peers.sort_by(|a, b| a.ip.cmp(&b.ip));
    PresentStatus {
        note_id: doc.note_id.clone(),
        title: doc.title.clone(),
        url: format!("http://{}:{}/", running.ip, running.port),
        ip: running.ip.clone(),
        port: running.port,
        rev: doc.rev,
        origin: doc.origin.clone(),
        html: doc.html.clone(),
        max_editors: doc.max_editors,
        peers,
        requests: doc
            .requests
            .iter()
            .map(|ip| PresentRequest { ip: ip.clone() })
            .collect(),
        notices: std::mem::take(&mut doc.owner_notices),
    }
}

fn fresh_doc(note_id: String, title: String, html: String, max_editors: u32) -> PresentDoc {
    PresentDoc {
        note_id,
        title,
        html,
        rev: 1,
        origin: "owner".into(),
        max_editors: clamp_max(max_editors),
        peers: HashMap::new(),
        requests: Vec::new(),
        editors: HashMap::new(),
        rejected: HashSet::new(),
        notices: HashMap::new(),
        owner_notices: Vec::new(),
        nickname: String::new(),
    }
}

pub fn start(note_id: String, title: String, html: String, max_editors: u32) -> Result<PresentStatus, String> {
    let mut guard = lock_in(slot());
    if let Some(running) = guard.as_ref() {
        if !running.shutdown.load(Ordering::SeqCst) {
            let mut doc = lock_in(&running.doc);
            let switched = doc.note_id != note_id;
            doc.note_id = note_id;
            doc.title = title;
            if doc.html != html {
                doc.html = html;
                doc.rev = doc.rev.saturating_add(1);
                doc.origin = "owner".into();
            }
            doc.max_editors = clamp_max(max_editors);
            if switched {
                doc.peers.clear();
                doc.requests.clear();
                doc.editors.clear();
                doc.rejected.clear();
                doc.notices.clear();
                doc.owner_notices.clear();
            }
            prune(&mut doc);
            return Ok(status_of(running, &mut doc));
        }
    }

    let (listener, port) = bind_listener()?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let shutdown = Arc::new(AtomicBool::new(false));
    let doc = Arc::new(Mutex::new(fresh_doc(note_id, title, html, max_editors)));
    let ip = lan_ipv4();
    let running = Running {
        shutdown: shutdown.clone(),
        doc: doc.clone(),
        ip: ip.clone(),
        port,
    };
    let page_ip = ip.clone();
    std::thread::Builder::new()
        .name("notebook-present".into())
        .spawn(move || serve_loop(listener, shutdown, doc, page_ip))
        .map_err(|e| e.to_string())?;
    let status = {
        let mut doc = lock_in(&running.doc);
        status_of(&running, &mut doc)
    };
    *guard = Some(running);
    Ok(status)
}

pub fn update(title: String, html: String) -> Result<(), String> {
    let guard = lock_in(slot());
    let Some(running) = guard.as_ref() else {
        return Ok(());
    };
    if running.shutdown.load(Ordering::SeqCst) {
        return Ok(());
    }
    let mut doc = lock_in(&running.doc);
    if doc.title == title && doc.html == html {
        return Ok(());
    }
    doc.title = title;
    doc.html = html;
    doc.rev = doc.rev.saturating_add(1);
    doc.origin = "owner".into();
    Ok(())
}

pub fn set_max_editors(max_editors: u32) -> Result<(), String> {
    let guard = lock_in(slot());
    let Some(running) = guard.as_ref() else {
        return Ok(());
    };
    let mut doc = lock_in(&running.doc);
    doc.max_editors = clamp_max(max_editors);
    while doc.editors.len() > doc.max_editors as usize {
        let Some(ip) = doc.editors.keys().next().cloned() else {
            break;
        };
        drop_editor(&mut doc, &ip);
        doc.notices.insert(ip, "笔记本人已停止你的编辑".into());
    }
    Ok(())
}

pub fn decide(ip: String, allow: bool) -> Result<(), String> {
    let guard = lock_in(slot());
    let Some(running) = guard.as_ref() else {
        return Err("演示未开始".into());
    };
    let mut doc = lock_in(&running.doc);
    if allow && !doc.editors.contains_key(&ip) && doc.editors.len() >= doc.max_editors as usize {
        return Err("同时编辑人数已满".into());
    }
    doc.requests.retain(|item| item != &ip);
    if allow {
        if !doc.editors.contains_key(&ip) {
            let color = next_color(&doc);
            doc.editors.insert(ip.clone(), color);
        }
        doc.rejected.remove(&ip);
        let color = doc.editors.get(&ip).cloned().unwrap_or_default();
        doc.peers.insert(
            ip,
            Peer {
                mode: "edit".into(),
                color,
                last_seen: now_ms(),
            },
        );
    } else {
        doc.editors.remove(&ip);
        doc.rejected.insert(ip);
    }
    Ok(())
}

fn drop_editor(doc: &mut PresentDoc, ip: &str) {
    doc.editors.remove(ip);
    doc.requests.retain(|item| item != ip);
    if let Some(peer) = doc.peers.get_mut(ip) {
        peer.mode = "view".into();
        peer.color.clear();
    }
}

pub fn revoke(target: Option<String>) -> Result<(), String> {
    let guard = lock_in(slot());
    let Some(running) = guard.as_ref() else {
        return Err("演示未开始".into());
    };
    let mut doc = lock_in(&running.doc);
    let ips: Vec<String> = match target {
        Some(ip) => vec![ip],
        None => doc.editors.keys().cloned().collect(),
    };
    for ip in ips {
        drop_editor(&mut doc, &ip);
        doc.notices.insert(ip, "笔记本人已停止你的编辑".into());
    }
    Ok(())
}

pub fn set_nickname(nickname: String) {
    let guard = lock_in(slot());
    let Some(running) = guard.as_ref() else {
        return;
    };
    let mut doc = lock_in(&running.doc);
    doc.nickname = nickname.trim().chars().take(24).collect();
}

pub fn stop() {
    let mut guard = lock_in(slot());
    if let Some(running) = guard.take() {
        running.shutdown.store(true, Ordering::SeqCst);
    }
}

pub fn status() -> Option<PresentStatus> {
    let guard = lock_in(slot());
    let running = guard.as_ref()?;
    if running.shutdown.load(Ordering::SeqCst) {
        return None;
    }
    let mut doc = lock_in(&running.doc);
    prune(&mut doc);
    Some(status_of(running, &mut doc))
}

fn bind_listener() -> Result<(TcpListener, u16), String> {
    for port in 17321..17341u16 {
        if let Ok(listener) = TcpListener::bind(("0.0.0.0", port)) {
            return Ok((listener, port));
        }
    }
    Err("没有可用端口，无法开始演示".into())
}

fn serve_loop(listener: TcpListener, shutdown: Arc<AtomicBool>, doc: Arc<Mutex<PresentDoc>>, lan_ip: String) {
    while !shutdown.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let doc = doc.clone();
                let lan_ip = lan_ip.clone();
                std::thread::spawn(move || {
                    let _ = handle_client(stream, &doc, &lan_ip);
                });
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(40));
            }
            Err(_) => break,
        }
    }
}

struct HttpReq {
    method: String,
    path: String,
    range: Option<String>,
    body: Vec<u8>,
    peer: String,
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<HttpReq> {
    stream.set_read_timeout(Some(Duration::from_secs(8)))?;
    let peer = stream
        .peer_addr()
        .map(normalize_ip)
        .unwrap_or_else(|_| "0.0.0.0".into());
    let mut buf = Vec::new();
    let mut tmp = [0u8; 8192];
    let header_end = loop {
        if buf.len() > 65536 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "header"));
        }
        let n = stream.read(&mut tmp)?;
        if n == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "eof"));
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos;
        }
    };
    let header_text = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut lines = header_text.lines();
    let req_line = lines.next().unwrap_or("");
    let mut parts = req_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("/").split('?').next().unwrap_or("/").to_string();
    let mut content_length = 0usize;
    let mut range = None;
    for line in lines {
        if let Some((key, value)) = line.split_once(':') {
            if key.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap_or(0).min(8_000_000);
            } else if key.eq_ignore_ascii_case("range") {
                range = Some(value.trim().to_string());
            }
        }
    }
    let mut body = buf[header_end + 4..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut tmp)?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }
    body.truncate(content_length);
    Ok(HttpReq { method, path, range, body, peer })
}

fn handle_client(mut stream: TcpStream, doc: &Mutex<PresentDoc>, lan_ip: &str) -> std::io::Result<()> {
    let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
    let req = read_request(&mut stream)?;
    if req.method != "GET" && req.method != "HEAD" && req.method != "POST" {
        return write_bytes(&mut stream, "405 Method Not Allowed", "text/plain; charset=utf-8", b"Method Not Allowed", "");
    }
    if req.path == "/" || req.path == "/index.html" {
        let page = viewer_html(lan_ip);
        return write_bytes(&mut stream, "200 OK", "text/html; charset=utf-8", page.as_bytes(), "");
    }
    if req.path == "/api/note" && req.method == "GET" {
        let body = note_json(&mut lock_in(doc), &req.peer);
        return write_bytes(&mut stream, "200 OK", "application/json; charset=utf-8", body.as_bytes(), "");
    }
    if req.path == "/api/request-edit" && req.method == "POST" {
        let (status, body) = request_edit(&mut lock_in(doc), &req.peer);
        return write_bytes(&mut stream, status, "text/plain; charset=utf-8", body.as_bytes(), "");
    }
    if req.path == "/api/edit" && req.method == "POST" {
        let (status, body) = apply_guest_edit(&mut lock_in(doc), &req.peer, &req.body);
        return write_bytes(&mut stream, status, "text/plain; charset=utf-8", body.as_bytes(), "");
    }
    if req.path == "/api/stop-edit" && req.method == "POST" {
        let (status, body) = stop_guest_edit(&mut lock_in(doc), &req.peer);
        return write_bytes(&mut stream, status, "text/plain; charset=utf-8", body.as_bytes(), "");
    }
    if req.path == "/favicon.ico" {
        return write_bytes(&mut stream, "204 No Content", "text/plain", b"", "");
    }
    if let Some(key) = req.path.strip_prefix("/media/") {
        let key = percent_decode_key(key);
        if key_ok(&key) {
            if let Ok(path) = media::absolute_path_for_key(&key) {
                return write_file(&mut stream, &path, req.range.as_deref());
            }
        }
    }
    write_bytes(&mut stream, "404 Not Found", "text/plain; charset=utf-8", b"Not Found", "")
}

fn note_json(doc: &mut PresentDoc, ip: &str) -> String {
    observe(doc, ip);
    let role = role_for(doc, ip);
    let color = doc.editors.get(ip).cloned().unwrap_or_default();
    let font = if color.is_empty() { String::new() } else { font_for(&color).to_string() };
    let notice = doc.notices.remove(ip).unwrap_or_default();
    serde_json::json!({
        "title": doc.title,
        "html": media::present_media_html(&doc.html),
        "rev": doc.rev,
        "origin": doc.origin,
        "role": role,
        "color": color,
        "font": font,
        "ip": ip,
        "notice": notice,
        "ownerName": doc.nickname,
    })
    .to_string()
}

fn request_edit(doc: &mut PresentDoc, ip: &str) -> (&'static str, String) {
    observe(doc, ip);
    if doc.editors.contains_key(ip) {
        return ("200 OK", "edit".into());
    }
    if doc.editors.len() >= doc.max_editors as usize {
        return ("409 Conflict", "编辑人数已满".into());
    }
    doc.rejected.remove(ip);
    if !doc.requests.iter().any(|item| item == ip) {
        doc.requests.push(ip.to_string());
    }
    ("200 OK", "pending".into())
}

fn stop_guest_edit(doc: &mut PresentDoc, ip: &str) -> (&'static str, String) {
    observe(doc, ip);
    let was_editor = doc.editors.contains_key(ip);
    drop_editor(doc, ip);
    if was_editor {
        doc.owner_notices.push(format!("{ip} 已退出编辑"));
    }
    ("200 OK", "ok".into())
}

#[derive(Deserialize)]
struct EditBody {
    html: String,
}

fn apply_guest_edit(doc: &mut PresentDoc, ip: &str, body: &[u8]) -> (&'static str, String) {
    observe(doc, ip);
    if !doc.editors.contains_key(ip) {
        return ("403 Forbidden", "还没有编辑权限".into());
    }
    let Ok(parsed) = serde_json::from_slice::<EditBody>(body) else {
        return ("400 Bad Request", "无法读取正文".into());
    };
    let html = restore_media_html(&parsed.html);
    if doc.html != html {
        doc.html = html;
        doc.rev = doc.rev.saturating_add(1);
        doc.origin = ip.to_string();
    }
    ("200 OK", "ok".into())
}

fn media_src_res() -> &'static [regex::Regex; 2] {
    static RE: OnceLock<[regex::Regex; 2]> = OnceLock::new();
    RE.get_or_init(|| {
        [
            regex::Regex::new(r#"(?i)\bsrc\s*=\s*"(?:https?://[^"]*)?/media/([^"]+)""#)
                .expect("present media src"),
            regex::Regex::new(r#"(?i)\bsrc\s*=\s*'(?:https?://[^']*)?/media/([^']+)'"#)
                .expect("present media src"),
        ]
    })
}

fn restore_media_html(html: &str) -> String {
    let res = media_src_res();
    let double = res[0].replace_all(html, |caps: &regex::Captures| {
        let key = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        format!("src=\"notebook-media://{key}\"")
    });
    res[1]
        .replace_all(&double, |caps: &regex::Captures| {
            let key = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            format!("src='notebook-media://{key}'")
        })
        .into_owned()
}

fn viewer_html(lan_ip: &str) -> String {
    VIEWER_HTML.replace("{{LAN_IP}}", lan_ip)
}

fn key_ok(key: &str) -> bool {
    !key.is_empty() && !key.contains("..") && !key.contains('/') && !key.contains('\\') && key.len() <= 180
}

fn percent_decode_key(src: &str) -> String {
    let bytes = src.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(v) = u8::from_str_radix(hex, 16) {
                    out.push(v);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn write_bytes(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8], extra: &str) -> std::io::Result<()> {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n{extra}\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    Ok(())
}

fn mime_of(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        _ => "application/octet-stream",
    }
}

fn parse_range(header: &str, len: u64) -> Option<(u64, u64)> {
    if len == 0 {
        return None;
    }
    let rest = header.trim().strip_prefix("bytes=")?;
    let (start_s, end_s) = rest.split_once('-')?;
    if start_s.is_empty() {
        let n: u64 = end_s.parse().ok()?;
        let start = len.saturating_sub(n.max(1));
        return Some((start, len - 1));
    }
    let start: u64 = start_s.parse().ok()?;
    if start >= len {
        return None;
    }
    let end = if end_s.is_empty() { len - 1 } else { end_s.parse::<u64>().ok()?.min(len - 1) };
    if end < start {
        return None;
    }
    Some((start, end))
}

fn write_file(stream: &mut TcpStream, path: &std::path::Path, range: Option<&str>) -> std::io::Result<()> {
    let file_len = std::fs::metadata(path)?.len();
    let mime = mime_of(path);
    let mut file = File::open(path)?;
    if let Some(header) = range {
        if let Some((start, end)) = parse_range(header, file_len) {
            let chunk = end - start + 1;
            file.seek(SeekFrom::Start(start))?;
            let head = format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Type: {mime}\r\nContent-Length: {chunk}\r\nContent-Range: bytes {start}-{end}/{file_len}\r\nAccept-Ranges: bytes\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n"
            );
            stream.write_all(head.as_bytes())?;
            return copy_n(stream, &mut file, chunk);
        }
    }
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {file_len}\r\nAccept-Ranges: bytes\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n"
    );
    stream.write_all(head.as_bytes())?;
    copy_n(stream, &mut file, file_len)
}

fn copy_n(stream: &mut TcpStream, file: &mut File, mut left: u64) -> std::io::Result<()> {
    let mut buf = [0u8; 65536];
    while left > 0 {
        let want = left.min(buf.len() as u64) as usize;
        let n = file.read(&mut buf[..want])?;
        if n == 0 {
            break;
        }
        stream.write_all(&buf[..n])?;
        left -= n as u64;
    }
    Ok(())
}

const VIEWER_HTML: &str = r##"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>实时演示</title>
<style>
  :root { color-scheme: light; }
  * { box-sizing: border-box; }
  body {
    margin: 0;
    min-height: 100vh;
    background:
      radial-gradient(1200px 480px at 10% -10%, #dbeafe 0%, transparent 55%),
      radial-gradient(900px 420px at 100% 0%, #e0e7ff 0%, transparent 50%),
      #f3f6fb;
    color: #111827;
    font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", sans-serif;
  }
  header {
    position: sticky; top: 0; z-index: 2;
    display: flex; align-items: center; justify-content: space-between; gap: 16px;
    padding: 14px 28px; background: rgba(255,255,255,.9); backdrop-filter: blur(12px);
    border-bottom: 1px solid #e5eaf2;
  }
  .brand { display: flex; align-items: center; gap: 8px; font-weight: 700; letter-spacing: -.02em; flex-wrap: wrap; }
  .src-ip { font-weight: 500; font-size: 13px; color: #64748b; }
  .dot { width: 9px; height: 9px; border-radius: 99px; background: #22c55e; box-shadow: 0 0 0 6px rgba(34,197,94,.15); }
  .tools { display: flex; align-items: center; gap: 10px; }
  #ask {
    height: 32px; padding: 0 12px; border-radius: 8px; border: 1px solid #bfdbfe;
    background: #eff6ff; color: #1d4ed8; cursor: pointer; font: inherit; font-size: 13px;
  }
  #ask:disabled { opacity: .7; cursor: default; }
  .notice {
    position: fixed; left: 50%; top: 72px; transform: translateX(-50%);
    background: #0f172a; color: #fff; padding: 10px 16px; border-radius: 10px;
    font-size: 13px; z-index: 5;
  }
  .state { font-size: 13px; color: #64748b; }
  .state.live { color: #15803d; }
  .state.down { color: #b91c1c; }
  main { max-width: 820px; margin: 28px auto 64px; padding: 0 20px; }
  article {
    background: #fff; border: 1px solid #e6ebf2; border-radius: 18px;
    padding: 28px 40px 48px; box-shadow: 0 18px 50px rgba(15, 23, 42, .06);
  }
  #editbar { display: flex; gap: 16px; align-items: center; margin-bottom: 14px; font-size: 13px; color: #475569; }
  #editbar[hidden] { display: none; }
  #editbar input { vertical-align: middle; }
  h1.doc-title { margin: 0 0 22px; font-size: 34px; line-height: 1.25; letter-spacing: -.03em; }
  .body { font-size: 16.5px; line-height: 1.75; min-height: 240px; outline: none; }
  .body[contenteditable="true"] { box-shadow: inset 0 0 0 1px #dbeafe; border-radius: 10px; padding: 8px; }
  .body p { margin: 0 0 .7em; }
  .body h1 { font-size: 1.7em; } .body h2 { font-size: 1.4em; } .body h3 { font-size: 1.2em; }
  .body img, .body video { max-width: 100%; height: auto; border-radius: 10px; display: block; margin: 12px 0; }
  .body pre { background: #0f172a; color: #e2e8f0; padding: 14px 16px; border-radius: 10px; overflow: auto; }
  .body code { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: .92em; }
  .body mark { color: inherit; }
  .body a { color: #2563eb; }
  footer { text-align: center; color: #94a3b8; font-size: 12px; margin-top: 18px; }
</style>
</head>
<body>
<header>
  <div class="brand"><span class="dot"></span> NoteBook 实时演示<span class="src-ip" data-ip="{{LAN_IP}}">（来源IP：{{LAN_IP}}）</span></div>
  <div class="tools">
    <button id="ask" type="button">申请同步编辑</button>
    <div id="state" class="state">正在连接…</div>
  </div>
</header>
<div id="notice" class="notice" hidden></div>
<main>
  <article>
    <div id="editbar" hidden>
      <label>文字颜色 <input id="fore" type="color" value="#111827" /></label>
      <label>背景颜色 <input id="back" type="color" value="#fde68a" /></label>
      <span id="mine"></span>
    </div>
    <h1 id="title" class="doc-title">加载中</h1>
    <div id="body" class="body"></div>
  </article>
  <footer>不过外网不上云，局域网本地笔记，只把这一篇笔记分享给身边人</footer>
</main>
<script>
let rev = -1;
let role = "view";
let myIp = "";
let myColor = "#7c3aed";
let myFont = "Georgia, serif";
let applying = false;
const body = document.getElementById("body");
const ask = document.getElementById("ask");
const editbar = document.getElementById("editbar");
const state = document.getElementById("state");
const mine = document.getElementById("mine");
document.execCommand("styleWithCSS", false, "true");

function paintAsk() {
  ask.disabled = false;
  if (role === "edit") { ask.textContent = "停止编辑"; }
  else if (role === "pending") { ask.textContent = "已申请，等待同意"; ask.disabled = true; }
  else if (role === "rejected") { ask.textContent = "申请被拒绝，再次申请"; }
  else if (role === "full") { ask.textContent = "编辑人数已满"; ask.disabled = true; }
  else ask.textContent = "申请同步编辑";
  editbar.hidden = role !== "edit";
  body.contentEditable = role === "edit" ? "true" : "false";
  mine.textContent = role === "edit" ? ("你的编辑会用单独字体标成 " + myIp) : "";
  if (role === "edit") { mine.style.color = myColor; mine.style.fontFamily = myFont; }
}

function caretOffset(root) {
  const sel = window.getSelection();
  if (!sel || !sel.rangeCount || !root.contains(sel.anchorNode)) return null;
  const range = sel.getRangeAt(0);
  const pre = range.cloneRange();
  pre.selectNodeContents(root);
  pre.setEnd(range.endContainer, range.endOffset);
  return pre.toString().length;
}
function restoreCaret(root, offset) {
  if (offset == null) return;
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  let left = offset;
  let node = walker.nextNode();
  while (node) {
    const len = node.nodeValue.length;
    if (left <= len) {
      const range = document.createRange();
      range.setStart(node, Math.max(0, left));
      range.collapse(true);
      const sel = window.getSelection();
      sel.removeAllRanges();
      sel.addRange(range);
      return;
    }
    left -= len;
    node = walker.nextNode();
  }
}
function wrapInserted(text) {
  if (!text || role !== "edit") return;
  const sel = window.getSelection();
  if (!sel || !sel.rangeCount) return;
  const node = sel.anchorNode;
  if (!node || node.nodeType !== 3) return;
  const offset = sel.anchorOffset;
  const start = offset - text.length;
  if (start < 0) return;
  const value = node.nodeValue || "";
  if (value.slice(start, offset) !== text) return;
  const parent = node.parentElement;
  if (parent && parent.dataset.collabIp === myIp && parent.childNodes.length === 1 && parent.textContent === text) return;
  const range = document.createRange();
  try {
    range.setStart(node, start);
    range.setEnd(node, offset);
    const span = document.createElement("span");
    span.dataset.collabIp = myIp;
    span.dataset.collabFont = myFont;
    span.className = "nb-collab";
    span.style.color = myColor;
    span.style.fontFamily = myFont;
    span.title = myIp;
    range.surroundContents(span);
    const after = document.createRange();
    after.selectNodeContents(span);
    after.collapse(false);
    sel.removeAllRanges();
    sel.addRange(after);
  } catch (e) {}
}
let pushTimer = 0;
function schedulePush() {
  if (role !== "edit" || applying) return;
  clearTimeout(pushTimer);
  pushTimer = setTimeout(async () => {
    try {
      await fetch("/api/edit", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ html: body.innerHTML })
      });
    } catch (e) {}
  }, 280);
}
body.addEventListener("beforeinput", (e) => {
  if (role !== "edit") e.preventDefault();
});
body.addEventListener("input", (e) => {
  if (role !== "edit" || applying) return;
  if (e.inputType === "insertText" && e.data) wrapInserted(e.data);
  if (e.inputType === "insertFromPaste") {
    const sel = window.getSelection();
    const node = sel && sel.anchorNode;
    if (node && node.nodeType === 3 && !(node.parentElement && node.parentElement.dataset.collabIp === myIp)) {
      const span = document.createElement("span");
      span.dataset.collabIp = myIp;
      span.dataset.collabFont = myFont;
      span.className = "nb-collab";
      span.style.color = myColor;
      span.style.fontFamily = myFont;
      span.title = myIp;
      node.parentNode.insertBefore(span, node);
      span.appendChild(node);
    }
  }
  schedulePush();
});
body.addEventListener("compositionend", (e) => {
  if (role !== "edit") return;
  wrapInserted(e.data || "");
  schedulePush();
});
document.getElementById("fore").addEventListener("input", (e) => {
  body.focus();
  document.execCommand("styleWithCSS", false, "true");
  document.execCommand("foreColor", false, e.target.value);
  schedulePush();
});
document.getElementById("back").addEventListener("input", (e) => {
  body.focus();
  document.execCommand("styleWithCSS", false, "true");
  document.execCommand("hiliteColor", false, e.target.value);
  schedulePush();
});
function showNotice(text) {
  if (!text) return;
  const n = document.getElementById("notice");
  n.hidden = false;
  n.textContent = text;
  clearTimeout(showNotice.timer);
  showNotice.timer = setTimeout(() => { n.hidden = true; }, 4200);
}
ask.addEventListener("click", async () => {
  if (ask.disabled) return;
  try {
    if (role === "edit") {
      await fetch("/api/stop-edit", { method: "POST" });
      showNotice("已退出编辑");
    } else {
      await fetch("/api/request-edit", { method: "POST" });
    }
  } catch (e) {}
  pollOnce();
});
let fails = 0;
let polling = false;
async function tick() {
  try {
    const res = await fetch("/api/note", { cache: "no-store" });
    if (!res.ok) throw new Error("bad");
    const data = await res.json();
    if (typeof data.rev !== "number" || typeof data.html !== "string") throw new Error("bad");
    role = data.role || "view";
    myIp = data.ip || myIp;
    if (data.color) myColor = data.color;
    if (data.font) myFont = data.font;
    if (data.notice) showNotice(data.notice);
    const src = document.querySelector(".src-ip");
    const ownerName = (data.ownerName || "").trim();
    if (src && ownerName) src.textContent = "（来源：" + ownerName + " · " + (src.dataset.ip || "") + "）";
    const sameEcho = role === "edit" && data.origin === myIp;
    if (data.rev !== rev && !sameEcho) {
      const offset = caretOffset(body);
      applying = true;
      body.innerHTML = data.html || "<p></p>";
      applying = false;
      if (role === "edit") restoreCaret(body, offset);
    }
    if (data.rev !== rev || !sameEcho) {
      document.getElementById("title").textContent = data.title || "未命名笔记";
      if (data.rev !== rev && sameEcho) {
        /* 自己刚提交的版本，不重绘正文 */
      } else if (data.rev === rev && body.innerHTML === "") {
        body.innerHTML = data.html || "<p></p>";
      }
    }
    rev = data.rev;
    document.title = (data.title || "笔记") + " · 实时演示";
    fails = 0;
    if (state.textContent !== "实时同步中") {
      state.textContent = "实时同步中";
      state.className = "state live";
    }
    paintAsk();
  } catch (e) {
    fails += 1;
    if (fails >= 3 && state.textContent !== "连接中断，正在重试") {
      state.textContent = "连接中断，正在重试";
      state.className = "state down";
    }
  }
}
async function pollOnce() {
  if (polling) return;
  polling = true;
  try { await tick(); } finally { polling = false; }
}
async function pollLoop() {
  await pollOnce();
  setTimeout(pollLoop, 1200);
}
pollLoop();
</script>
</body>
</html>
"##;

#[cfg(test)]
mod tests {
    use super::restore_media_html;

    #[test]
    fn restores_present_media_src_without_panic() {
        let html = r#"<p><img src="/media/a.png"><img src="http://10.0.0.8:17321/media/b.jpg"><img src='https://10.0.0.8:17321/media/c.webp'></p>"#;
        let out = restore_media_html(html);
        assert!(out.contains("notebook-media://a.png"), "{out}");
        assert!(out.contains("notebook-media://b.jpg"), "{out}");
        assert!(out.contains("notebook-media://c.webp"), "{out}");
        assert!(!out.contains("/media/"), "{out}");
    }
}
