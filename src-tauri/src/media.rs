use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use uuid::Uuid;

pub const MEDIA_SCHEME: &str = "notebook-media://";

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImportedMedia {
    /// 存于应用 media 目录下的文件名，如 `a1b2.png`
    pub storage_key: String,
    /// 绝对路径，供前端 convertFileSrc 展示
    pub absolute_path: String,
    /// 写入 HTML 的 src，形如 notebook-media://a1b2.png
    pub media_src: String,
}

/// 媒体文件与笔记的绑定关系
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MediaBinding {
    /// 存于 media 目录下的文件名
    pub storage_key: String,
    /// 引用此媒体的笔记 ID 列表（导入恢复时使用）
    pub note_ids: Vec<String>,
}

/// 媒体导出清单
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MediaExportManifest {
    pub version: String,
    pub bindings: Vec<MediaBinding>,
}

pub fn app_data_dir() -> Result<PathBuf, String> {
    let dir = dirs::data_local_dir()
        .ok_or_else(|| "无法解析本地数据目录".to_string())?
        .join("com.tal.notebook");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

pub fn media_dir() -> Result<PathBuf, String> {
    let dir = app_data_dir()?.join("media");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

pub fn clear_media_dir() -> Result<(), String> {
    let dir = media_dir()?;
    if dir.exists() {
        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_file() {
                fs::remove_file(path).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

fn ext_from_path(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "bin".to_string())
}

fn ext_from_mime(mime: &str) -> Option<&'static str> {
    match mime {
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/svg+xml" => Some("svg"),
        "video/mp4" => Some("mp4"),
        "video/webm" => Some("webm"),
        "video/quicktime" => Some("mov"),
        _ => None,
    }
}

fn is_allowed_ext(ext: &str) -> bool {
    matches!(
        ext,
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "svg" | "mp4" | "webm" | "mov"
    )
}

fn new_storage_key(ext: &str) -> String {
    format!("{}.{}", Uuid::new_v4(), ext)
}

pub fn media_src_for_key(key: &str) -> String {
    format!("{MEDIA_SCHEME}{key}")
}

pub fn storage_key_from_media_src(src: &str) -> Option<String> {
    src.strip_prefix(MEDIA_SCHEME)
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty() && !k.contains("..") && !k.contains('/'))
}

pub fn safe_media_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 180 || name.starts_with('.') {
        return false;
    }
    if name.contains("..") || name.contains('/') || name.contains('\\') {
        return false;
    }
    let ext = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    is_allowed_ext(&ext)
}

pub fn media_file_exists(key: &str) -> bool {
    if !safe_media_name(key) {
        return false;
    }
    media_dir()
        .ok()
        .map(|root| root.join(key).is_file())
        .unwrap_or(false)
}

pub fn read_export_media(keys: &[String]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let root = media_dir()?;
    let mut missing = Vec::new();
    let mut files = Vec::new();
    for key in keys {
        if !safe_media_name(key) {
            return Err(format!("无效的媒体文件名: {key}"));
        }
        match fs::read(root.join(key)) {
            Ok(bytes) => files.push((key.clone(), bytes)),
            Err(_) => missing.push(key.clone()),
        }
    }
    if !missing.is_empty() {
        return Err(format!(
            "这些图片或视频已不在本地，无法完整导出：{}",
            missing.join("、")
        ));
    }
    Ok(files)
}

pub fn read_sidecar_media(tbook_path: &Path) -> Result<HashMap<String, Vec<u8>>, String> {
    let mut out = HashMap::new();
    for dir in import_media_dir_candidates(tbook_path) {
        if !dir.is_dir() {
            continue;
        }
        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !safe_media_name(name) {
                continue;
            }
            let bytes = fs::read(&path).map_err(|e| e.to_string())?;
            out.entry(name.to_string()).or_insert(bytes);
        }
    }
    Ok(out)
}

pub fn install_media_files(files: &HashMap<String, Vec<u8>>) -> Result<(), String> {
    if files.is_empty() {
        return Ok(());
    }
    let root = media_dir()?;
    let mut staged = Vec::new();
    for (name, bytes) in files {
        if !safe_media_name(name) {
            return Err(format!("无效的媒体文件名: {name}"));
        }
        let tmp = root.join(format!(".{name}.importing"));
        fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
        staged.push((tmp, root.join(name)));
    }
    for (tmp, dest) in staged {
        fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn absolute_path_for_key(key: &str) -> Result<PathBuf, String> {
    if key.contains("..") || key.contains('/') || key.contains('\\') {
        return Err("无效的媒体文件名".to_string());
    }
    let path = media_dir()?.join(key);
    if !path.is_file() {
        return Err(format!("媒体文件不存在: {key}"));
    }
    Ok(path)
}

pub fn import_media_from_path(source_path: &str) -> Result<ImportedMedia, String> {
    let src = Path::new(source_path);
    if !src.is_file() {
        return Err("文件不存在".to_string());
    }
    let ext = ext_from_path(src);
    if !is_allowed_ext(&ext) {
        return Err("不支持的图片或视频格式".to_string());
    }
    let key = new_storage_key(&ext);
    let dest = media_dir()?.join(&key);
    fs::copy(src, &dest).map_err(|e| e.to_string())?;
    let absolute_path = dest
        .canonicalize()
        .unwrap_or(dest)
        .to_string_lossy()
        .to_string();
    Ok(ImportedMedia {
        media_src: media_src_for_key(&key),
        storage_key: key,
        absolute_path,
    })
}

pub fn import_media_bytes(data: Vec<u8>, extension: String) -> Result<ImportedMedia, String> {
    let ext = extension.trim().trim_start_matches('.').to_ascii_lowercase();
    if !is_allowed_ext(&ext) {
        return Err("不支持的图片或视频格式".to_string());
    }
    let key = new_storage_key(&ext);
    let dest = media_dir()?.join(&key);
    fs::write(&dest, &data).map_err(|e| e.to_string())?;
    let absolute_path = dest
        .canonicalize()
        .unwrap_or(dest)
        .to_string_lossy()
        .to_string();
    Ok(ImportedMedia {
        media_src: media_src_for_key(&key),
        storage_key: key,
        absolute_path,
    })
}

pub fn import_media_bytes_with_mime(data: Vec<u8>, mime: Option<String>) -> Result<ImportedMedia, String> {
    let ext = mime
        .as_deref()
        .and_then(ext_from_mime)
        .unwrap_or("bin");
    if ext == "bin" {
        return Err("无法识别文件类型".to_string());
    }
    import_media_bytes(data, ext.to_string())
}

pub fn copy_media_file_to_app(key: &str, source: &Path) -> Result<(), String> {
    if key.contains("..") || key.contains('/') || key.contains('\\') {
        return Err("无效的媒体文件名".to_string());
    }
    if !source.is_file() {
        return Err(format!("备份中缺少媒体: {key}"));
    }
    let dest = media_dir()?.join(key);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::copy(source, &dest).map_err(|e| e.to_string())?;
    Ok(())
}

fn src_attr_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(?i)\bsrc\s*=\s*(?:"([^"]+)"|'([^']+)')"#).unwrap())
}

fn media_key_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)(?:notebook-media://|[/\\]media[/\\])([A-Za-z0-9][A-Za-z0-9._-]{0,180}\.(?:jpg|jpeg|png|gif|webp|svg|mp4|webm|mov))",
        )
        .unwrap()
    })
}

fn percent_decode(src: &str) -> String {
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

/// 从单个 src 里取出媒体文件名。兼容 `notebook-media://` 与 asset 本地路径。
pub fn key_from_src(src: &str) -> Option<String> {
    let decoded = percent_decode(src);
    media_key_re()
        .captures(&decoded)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        .filter(|k| !k.contains(".."))
}

pub fn collect_media_keys_from_html(html: &str) -> Vec<String> {
    let mut keys: Vec<String> = src_attr_re()
        .captures_iter(html)
        .filter_map(|c| c.get(1).or_else(|| c.get(2)).map(|m| m.as_str().to_string()))
        .filter_map(|src| key_from_src(&src))
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

/// 把正文里的媒体 src 收成 `notebook-media://最终文件名`。`renames` 只放被改名的项。
pub fn canonicalize_media_html(html: &str, renames: &HashMap<String, String>) -> String {
    src_attr_re()
        .replace_all(html, |caps: &regex::Captures| {
            let quote_src = caps.get(1).or_else(|| caps.get(2));
            let Some(src_match) = quote_src else {
                return caps[0].to_string();
            };
            let src = src_match.as_str();
            let Some(key) = key_from_src(src) else {
                return caps[0].to_string();
            };
            let final_key = renames.get(&key).cloned().unwrap_or(key);
            let quote = if caps.get(1).is_some() { '"' } else { '\'' };
            format!("src={quote}{MEDIA_SCHEME}{final_key}{quote}")
        })
        .into_owned()
}

/// 演示页使用的相对地址，浏览器向本机演示服务拉文件。
pub fn present_media_html(html: &str) -> String {
    src_attr_re()
        .replace_all(html, |caps: &regex::Captures| {
            let quote_src = caps.get(1).or_else(|| caps.get(2));
            let Some(src_match) = quote_src else {
                return caps[0].to_string();
            };
            let Some(key) = key_from_src(src_match.as_str()) else {
                return caps[0].to_string();
            };
            let quote = if caps.get(1).is_some() { '"' } else { '\'' };
            format!("src={quote}/media/{key}{quote}")
        })
        .into_owned()
}

fn files_identical(a: &Path, b: &Path) -> bool {
    let Ok(ma) = fs::metadata(a) else { return false };
    let Ok(mb) = fs::metadata(b) else { return false };
    if ma.len() != mb.len() || !ma.is_file() || !mb.is_file() {
        return false;
    }
    let Ok(mut fa) = File::open(a) else { return false };
    let Ok(mut fb) = File::open(b) else { return false };
    let mut ba = [0u8; 65536];
    let mut bb = [0u8; 65536];
    loop {
        let na = fa.read(&mut ba).unwrap_or(0);
        let nb = fb.read(&mut bb).unwrap_or(0);
        if na != nb || ba[..na] != bb[..nb] {
            return false;
        }
        if na == 0 {
            return true;
        }
    }
}

fn split_stem_ext(key: &str) -> (String, String) {
    match key.rfind('.') {
        Some(i) if i > 0 => (key[..i].to_string(), key[i + 1..].to_string()),
        _ => (key.to_string(), String::new()),
    }
}

fn dated_name(key: &str, now: DateTime<Utc>, n: u32) -> String {
    let stamp = now.format("%Y%m%d_%H%M%S");
    let (stem, ext) = split_stem_ext(key);
    let suffix = if n <= 1 {
        stamp.to_string()
    } else {
        format!("{stamp}_{n}")
    };
    if ext.is_empty() {
        format!("{stem}_{suffix}")
    } else {
        format!("{stem}_{suffix}.{ext}")
    }
}

/// 同名且内容相同则沿用原名；同名但内容不同则加日期，不覆盖已有文件。返回最终文件名。
pub fn place_export_file(dir: &Path, key: &str, src: &Path, now: DateTime<Utc>) -> Result<String, String> {
    if key.contains("..") || key.contains('/') || key.contains('\\') {
        return Err("无效的媒体文件名".to_string());
    }
    if !src.is_file() {
        return Err(format!("媒体文件不存在: {key}"));
    }
    let dest = dir.join(key);
    if !dest.exists() {
        fs::copy(src, &dest).map_err(|e| e.to_string())?;
        return Ok(key.to_string());
    }
    if dest.is_file() && files_identical(src, &dest) {
        return Ok(key.to_string());
    }
    for n in 1..50 {
        let name = dated_name(key, now, n);
        let path = dir.join(&name);
        if path.exists() {
            if path.is_file() && files_identical(src, &path) {
                return Ok(name);
            }
            continue;
        }
        fs::copy(src, &path).map_err(|e| e.to_string())?;
        return Ok(name);
    }
    Err(format!("无法为 {key} 分配不重复的文件名"))
}

pub fn export_media_dir_for(tbook_path: &Path) -> PathBuf {
    let parent = tbook_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = tbook_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    parent.join(format!("{stem}.media"))
}

/// 把笔记引用的媒体复制到与 .tbook 同级的 `文件名.media` 目录。返回需要改写的文件名。
pub fn export_note_media(tbook_path: &Path, keys: &[String]) -> Result<HashMap<String, String>, String> {
    let mut renames = HashMap::new();
    if keys.is_empty() {
        return Ok(renames);
    }
    let dir = export_media_dir_for(tbook_path);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let src_root = media_dir()?;
    let now = Utc::now();
    let mut copied: Vec<String> = Vec::new();
    for key in keys {
        let src = src_root.join(key);
        if !src.is_file() {
            continue;
        }
        let final_name = place_export_file(&dir, key, &src, now)?;
        if &final_name != key {
            renames.insert(key.clone(), final_name.clone());
        }
        copied.push(final_name);
    }
    let manifest_path = dir.join("media_manifest.json");
    let manifest = serde_json::json!({
        "version": "1.2.0",
        "files": copied,
    });
    fs::write(manifest_path, serde_json::to_string_pretty(&manifest).unwrap_or_else(|_| "{}".into()))
        .map_err(|e| e.to_string())?;
    Ok(renames)
}

pub fn import_media_dir_candidates(tbook_path: &Path) -> Vec<PathBuf> {
    let parent = tbook_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = tbook_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let mut dirs = vec![parent.join(format!("{stem}.media"))];
    let plain = parent.join(stem.as_ref());
    if plain.join("media_manifest.json").is_file() {
        dirs.push(plain);
    }
    dirs
}

pub fn copy_imported_media_dir(dir: &Path) -> Result<(), String> {
    if !dir.is_dir() {
        return Ok(());
    }
    let dest_root = media_dir()?;
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name == "media_manifest.json" || name.starts_with('.') {
            continue;
        }
        let ext = ext_from_path(&path);
        if !is_allowed_ext(&ext) {
            continue;
        }
        if name.contains("..") || name.contains('/') || name.contains('\\') {
            continue;
        }
        fs::copy(&path, dest_root.join(name)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedMedia {
    pub storage_key: String,
    pub absolute_path: String,
}

pub fn resolve_media_keys(keys: &[String]) -> Result<Vec<ResolvedMedia>, String> {
    let root = media_dir()?;
    let mut out = Vec::new();
    for key in keys {
        if key.contains("..") || key.contains('/') || key.contains('\\') || key.is_empty() {
            continue;
        }
        let path = root.join(key);
        if path.is_file() {
            out.push(ResolvedMedia {
                storage_key: key.clone(),
                absolute_path: path.to_string_lossy().to_string(),
            });
        }
    }
    Ok(out)
}

pub fn collect_all_media_keys(conn: &rusqlite::Connection) -> Result<Vec<String>, String> {
    use rusqlite::OptionalExtension;
    let mut stmt = conn
        .prepare("SELECT content_html FROM notes WHERE deleted_at IS NULL AND content_html IS NOT NULL")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| r.get::<_, Option<String>>(0))
        .map_err(|e| e.to_string())?;
    let mut all = Vec::new();
    for row in rows {
        if let Some(Some(html)) = row.optional().map_err(|e| e.to_string())? {
            all.extend(collect_media_keys_from_html(&html));
        }
    }
    all.sort();
    all.dedup();
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn parse_media_src() {
        assert_eq!(
            storage_key_from_media_src("notebook-media://abc.png"),
            Some("abc.png".to_string())
        );
    }

    #[test]
    fn extracts_scheme_and_asset_url() {
        let html = r#"<img src="notebook-media://a.png"><img src="http://asset.localhost/%2FUsers%2Ftal%2Fmedia%2Fb.jpg"><video src='C:\data\media\c.mp4'></video>"#;
        let keys = collect_media_keys_from_html(html);
        assert_eq!(keys, vec!["a.png".to_string(), "b.jpg".to_string(), "c.mp4".to_string()]);
    }

    #[test]
    fn rewrites_conflicting_name_to_dated_file() {
        let dir = std::env::temp_dir().join(format!("nb-media-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let src_old = dir.join("src-old.png");
        let src_new = dir.join("src-new.png");
        fs::write(&src_old, b"old-bytes").unwrap();
        fs::write(&src_new, b"new-bytes-xxx").unwrap();
        let export_dir = dir.join("out");
        fs::create_dir_all(&export_dir).unwrap();
        fs::copy(&src_old, export_dir.join("pic.png")).unwrap();
        let now = Utc::now();
        let name = place_export_file(&export_dir, "pic.png", &src_new, now).unwrap();
        assert_ne!(name, "pic.png");
        assert!(name.starts_with("pic_"));
        assert!(name.ends_with(".png"));
        assert_eq!(fs::read(export_dir.join("pic.png")).unwrap(), b"old-bytes");
        assert_eq!(fs::read(export_dir.join(&name)).unwrap(), b"new-bytes-xxx");
        let mut renames = HashMap::new();
        renames.insert("pic.png".to_string(), name.clone());
        let html = r#"<img src="http://asset.localhost/%2FUsers%2Fme%2Fmedia%2Fpic.png">"#;
        let rewritten = canonicalize_media_html(html, &renames);
        assert_eq!(rewritten, format!(r#"<img src="notebook-media://{name}">"#));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn keeps_name_when_bytes_match() {
        let dir = std::env::temp_dir().join(format!("nb-media-same-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.png");
        fs::write(&src, b"same").unwrap();
        fs::write(dir.join("pic.png"), b"same").unwrap();
        let name = place_export_file(&dir, "pic.png", &src, Utc::now()).unwrap();
        assert_eq!(name, "pic.png");
        let _ = fs::remove_dir_all(&dir);
    }
}
