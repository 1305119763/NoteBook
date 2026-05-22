use regex::Regex;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const MEDIA_SCHEME: &str = "notebook-media://";

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImportedMedia {
    /// 存于应用 media 目录下的文件名，如 `a1b2.png`
    pub storage_key: String,
    /// 绝对路径，供前端 convertFileSrc 展示
    pub absolute_path: String,
    /// 写入 HTML 的 src，形如 notebook-media://a1b2.png
    pub media_src: String,
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

pub fn collect_media_keys_from_html(html: &str) -> Vec<String> {
    let re = Regex::new(&format!(r#"(?i){}([^"'\s>]+)"#, regex::escape(MEDIA_SCHEME))).unwrap();
    let mut keys: Vec<String> = re
        .captures_iter(html)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect();
    keys.sort();
    keys.dedup();
    keys
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

    #[test]
    fn parse_media_src() {
        assert_eq!(
            storage_key_from_media_src("notebook-media://abc.png"),
            Some("abc.png".to_string())
        );
    }
}
