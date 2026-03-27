use chrono::Utc;
use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FolderRow {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub sort_order: i64,
    pub created_at: String,
}

/// 废纸篓列表项（文件夹为一行含笔记数量；单独删除的笔记单独一行）
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TrashItemRow {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub deleted_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note_count: Option<i64>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NoteRow {
    pub id: String,
    pub folder_id: String,
    pub title: String,
    pub preview: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
    pub is_favorite: bool,
    /// 可统计正文字符数（排除图片/音视频/嵌入/代码块等后的纯文本，不含空白）。
    pub body_text_units: i64,
    /// 列表接口不查询此列，导出时填充；序列化时省略 `None`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_html: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EntryRow {
    pub id: String,
    pub note_id: String,
    pub title: String,
    pub body: String,
    pub sort_order: i64,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderImport {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub sort_order: i64,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteImport {
    pub id: String,
    pub folder_id: String,
    pub title: String,
    pub preview: Option<String>,
    #[serde(default)]
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub content_html: Option<String>,
    #[serde(default)]
    pub is_favorite: bool,
    #[serde(default)]
    pub body_text_units: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryImport {
    pub id: String,
    pub note_id: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub sort_order: i64,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TbookPayload {
    format: String,
    version: u32,
    exported_at: String,
    folders: Vec<FolderRow>,
    notes: Vec<NoteRow>,
    entries: Vec<EntryRow>,
}

pub fn db_path() -> Result<PathBuf, String> {
    let dir = dirs::data_local_dir()
        .ok_or_else(|| "无法解析本地数据目录".to_string())?
        .join("com.tal.notebook");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("notebook.db"))
}

pub fn open(_app: &tauri::AppHandle) -> Result<Connection, String> {
    let path = db_path()?;
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|e| e.to_string())?;
    migrate(&conn)?;
    Ok(conn)
}

/// 废纸篓保留天数，过期后笔记与文件夹从数据库永久删除。
pub const TRASH_RETENTION_DAYS: i64 = 30;

/// 删除已超过保留期的废纸篓条目（先笔记后文件夹，避免孤立数据）。
pub fn purge_expired_trash(conn: &Connection) -> Result<(), String> {
    let ver: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);
    if ver < 6 {
        return Ok(());
    }
    let cutoff = Utc::now() - chrono::Duration::days(TRASH_RETENTION_DAYS);
    let cutoff_s = cutoff.to_rfc3339();
    conn.execute(
        "DELETE FROM notes WHERE deleted_at IS NOT NULL AND deleted_at < ?1",
        params![cutoff_s],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM folders WHERE deleted_at IS NOT NULL AND deleted_at < ?1",
        params![cutoff_s],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn migrate(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS folders (
            id TEXT PRIMARY KEY NOT NULL,
            parent_id TEXT REFERENCES folders(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS notes (
            id TEXT PRIMARY KEY NOT NULL,
            folder_id TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            preview TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS entries (
            id TEXT PRIMARY KEY NOT NULL,
            note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            body TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_notes_folder ON notes(folder_id);
        CREATE INDEX IF NOT EXISTS idx_entries_note ON entries(note_id);
        CREATE INDEX IF NOT EXISTS idx_folders_parent ON folders(parent_id);
        ",
    )
    .map_err(|e| e.to_string())?;

    let ver: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);
    if ver < 2 {
        conn.execute("ALTER TABLE notes ADD COLUMN content_html TEXT", [])
            .map_err(|e| e.to_string())?;
        backfill_content_html_from_entries(conn)?;
        conn.execute("PRAGMA user_version = 2", [])
            .map_err(|e| e.to_string())?;
    }

    let ver2: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);
    if ver2 < 3 {
        flatten_folder_hierarchy(conn)?;
        dedupe_root_folder_names(conn)?;
        conn.execute(
            "ALTER TABLE notes ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| e.to_string())?;
        backfill_note_sort_order(conn)?;
        conn.execute("PRAGMA user_version = 3", [])
            .map_err(|e| e.to_string())?;
    }

    let ver3: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);
    if ver3 < 4 {
        dedupe_root_folder_names(conn)?;
        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS ux_folders_root_trim_name ON folders (trim(name)) WHERE parent_id IS NULL",
            [],
        )
        .map_err(|e| e.to_string())?;
        conn.execute("PRAGMA user_version = 4", [])
            .map_err(|e| e.to_string())?;
    }

    let ver4: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);
    if ver4 < 5 {
        conn.execute(
            "ALTER TABLE notes ADD COLUMN is_favorite INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| e.to_string())?;
        conn.execute("PRAGMA user_version = 5", [])
            .map_err(|e| e.to_string())?;
    }

    let ver5: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);
    if ver5 < 6 {
        conn.execute("ALTER TABLE folders ADD COLUMN deleted_at TEXT", [])
            .map_err(|e| e.to_string())?;
        conn.execute("ALTER TABLE notes ADD COLUMN deleted_at TEXT", [])
            .map_err(|e| e.to_string())?;
        conn.execute("PRAGMA user_version = 6", [])
            .map_err(|e| e.to_string())?;
    }

    let ver6: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);
    if ver6 < 7 {
        conn.execute(
            "ALTER TABLE notes ADD COLUMN body_text_units INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| e.to_string())?;
        backfill_body_text_units(conn)?;
        conn.execute("PRAGMA user_version = 7", [])
            .map_err(|e| e.to_string())?;
    }

    purge_expired_trash(conn)?;

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM folders", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if count == 0 {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO folders (id, parent_id, name, sort_order, created_at) VALUES (?1, NULL, ?2, 0, ?3)",
            params![id, "未分类", now],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn strip_html_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// 字数统计：不含空白字符（与 HTML 剥离后的可见文本一致）。
fn count_stat_text_from_plain(plain: &str) -> i64 {
    plain.chars().filter(|c| !c.is_whitespace()).count() as i64
}

/// 从 HTML 统计正文字数：去掉脚本/样式、图片、音视频、嵌入、代码块等后再剥标签计数。
fn count_stat_text_from_html(html: &str) -> i64 {
    let mut s = html.to_string();
    let block_patterns = [
        r"(?is)<script\b[^>]*>.*?</script>",
        r"(?is)<style\b[^>]*>.*?</style>",
        r"(?is)<noscript\b[^>]*>.*?</noscript>",
        r"(?is)<pre\b[^>]*>.*?</pre>",
        r"(?is)<video\b[^>]*>.*?</video>",
        r"(?is)<audio\b[^>]*>.*?</audio>",
        r"(?is)<iframe\b[^>]*>.*?</iframe>",
        r"(?is)<object\b[^>]*>.*?</object>",
        r"(?is)<svg\b[^>]*>.*?</svg>",
        r"(?is)<picture\b[^>]*>.*?</picture>",
        r"(?is)<canvas\b[^>]*>.*?</canvas>",
    ];
    for p in &block_patterns {
        if let Ok(re) = Regex::new(p) {
            s = re.replace_all(&s, "").to_string();
        }
    }
    let void_patterns = [
        r"(?is)<img\b[^>]*>",
        r"(?is)<embed\b[^>]*>",
        r"(?is)<input\b[^>]*>",
        r"(?is)<source\b[^>]*>",
        r"(?is)<track\b[^>]*>",
    ];
    for p in &void_patterns {
        if let Ok(re) = Regex::new(p) {
            s = re.replace_all(&s, "").to_string();
        }
    }
    if let Ok(re) = Regex::new(r"(?is)<code\b[^>]*>.*?</code>") {
        s = re.replace_all(&s, "").to_string();
    }
    let plain = strip_html_tags(&s);
    count_stat_text_from_plain(&plain)
}

fn backfill_body_text_units(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare("SELECT id, content_html FROM notes WHERE deleted_at IS NULL")
        .map_err(|e| e.to_string())?;
    let rows: Vec<(String, Option<String>)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);

    for (id, html_opt) in rows {
        let units = match html_opt.as_ref().map(|x| x.as_str()) {
            Some(h) if !h.trim().is_empty() => count_stat_text_from_html(h),
            _ => {
                let mut es = conn
                    .prepare(
                        "SELECT body FROM entries WHERE note_id = ?1 ORDER BY sort_order ASC, created_at ASC",
                    )
                    .map_err(|e| e.to_string())?;
                let bodies: Vec<String> = es
                    .query_map(params![id.clone()], |r| r.get(0))
                    .map_err(|e| e.to_string())?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| e.to_string())?;
                drop(es);
                let concat = bodies.join("\n");
                count_stat_text_from_plain(&concat)
            }
        };
        conn.execute(
            "UPDATE notes SET body_text_units = ?1 WHERE id = ?2",
            params![units, id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn backfill_content_html_from_entries(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare("SELECT id FROM notes")
        .map_err(|e| e.to_string())?;
    let ids: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);

    for nid in ids {
        let existing: Option<String> = conn
            .query_row(
                "SELECT content_html FROM notes WHERE id = ?1",
                params![nid],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if existing.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false) {
            continue;
        }
        let ents = list_entries(conn, nid.clone())?;
        if ents.is_empty() {
            continue;
        }
        let mut html = String::new();
        for e in ents {
            let t = html_escape(&e.title);
            let b = html_escape(&e.body).replace('\n', "<br>\n");
            html.push_str(&format!("<h3>{}</h3><p>{}</p>", t, b));
        }
        conn.execute(
            "UPDATE notes SET content_html = ?1 WHERE id = ?2",
            params![html, nid],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn flatten_folder_hierarchy(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare("SELECT id, parent_id FROM folders")
        .map_err(|e| e.to_string())?;
    let pairs: Vec<(String, Option<String>)> = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);
    let parent_of: HashMap<String, Option<String>> = pairs.into_iter().collect();

    fn root_of(parent_of: &HashMap<String, Option<String>>, mut id: String) -> String {
        loop {
            match parent_of.get(&id).cloned().flatten() {
                None => return id,
                Some(p) => id = p,
            }
        }
    }

    let mut stmt = conn
        .prepare("SELECT id, folder_id FROM notes")
        .map_err(|e| e.to_string())?;
    let note_pairs: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);

    for (nid, fid) in note_pairs {
        let r = root_of(&parent_of, fid.clone());
        if r != fid {
            conn.execute(
                "UPDATE notes SET folder_id = ?1 WHERE id = ?2",
                params![r, nid],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    conn.execute("DELETE FROM folders WHERE parent_id IS NOT NULL", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn dedupe_root_folder_names(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name FROM folders WHERE parent_id IS NULL ORDER BY sort_order, created_at",
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);

    let mut used: HashSet<String> = HashSet::new();
    for (id, name) in rows {
        let base = name.trim().to_string();
        let mut final_name = base.clone();
        if !used.contains(&final_name) {
            used.insert(final_name);
            continue;
        }
        let mut n = 2u32;
        loop {
            let candidate = format!("{} ({})", base, n);
            if !used.contains(&candidate) {
                final_name = candidate;
                break;
            }
            n += 1;
        }
        used.insert(final_name.clone());
        conn.execute(
            "UPDATE folders SET name = ?1 WHERE id = ?2",
            params![final_name, id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn backfill_note_sort_order(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare("SELECT DISTINCT folder_id FROM notes")
        .map_err(|e| e.to_string())?;
    let folder_ids: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);

    for fid in folder_ids {
        let mut stmt = conn
            .prepare("SELECT id FROM notes WHERE folder_id = ?1 ORDER BY updated_at DESC")
            .map_err(|e| e.to_string())?;
        let ids: Vec<String> = stmt
            .query_map(params![fid.clone()], |r| r.get(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        drop(stmt);
        for (i, nid) in ids.iter().enumerate() {
            conn.execute(
                "UPDATE notes SET sort_order = ?1 WHERE id = ?2",
                params![i as i64, nid],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn normalize_two_level_after_import(conn: &Connection) -> Result<(), String> {
    flatten_folder_hierarchy(conn)?;
    dedupe_root_folder_names(conn)?;
    backfill_note_sort_order(conn)?;
    Ok(())
}

fn folder_name_taken(
    conn: &Connection,
    parent_id: Option<&str>,
    name: &str,
    exclude_id: Option<&str>,
) -> Result<bool, String> {
    let n = name.trim();
    if n.is_empty() {
        return Ok(true);
    }
    let c: i64 = match (parent_id, exclude_id) {
        (None, None) => conn.query_row(
            "SELECT COUNT(*) FROM folders WHERE parent_id IS NULL AND TRIM(name) = ?1 AND deleted_at IS NULL",
            params![n],
            |r| r.get(0),
        ),
        (None, Some(ex)) => conn.query_row(
            "SELECT COUNT(*) FROM folders WHERE parent_id IS NULL AND TRIM(name) = ?1 AND id != ?2 AND deleted_at IS NULL",
            params![n, ex],
            |r| r.get(0),
        ),
        (Some(pid), None) => conn.query_row(
            "SELECT COUNT(*) FROM folders WHERE parent_id = ?1 AND TRIM(name) = ?2 AND deleted_at IS NULL",
            params![pid, n],
            |r| r.get(0),
        ),
        (Some(pid), Some(ex)) => conn.query_row(
            "SELECT COUNT(*) FROM folders WHERE parent_id = ?1 AND TRIM(name) = ?2 AND id != ?3 AND deleted_at IS NULL",
            params![pid, n, ex],
            |r| r.get(0),
        ),
    }
    .map_err(|e| e.to_string())?;
    Ok(c > 0)
}

fn alloc_unique_note_title(conn: &Connection, folder_id: &str, base: &str) -> Result<String, String> {
    let base = base.trim();
    if base.is_empty() {
        return Err("标题不能为空".to_string());
    }
    if !note_title_taken(conn, folder_id, base, None)? {
        return Ok(base.to_string());
    }
    let mut n = 2u32;
    loop {
        let candidate = format!("{} ({})", base, n);
        if !note_title_taken(conn, folder_id, &candidate, None)? {
            return Ok(candidate);
        }
        n += 1;
        if n > 10_000 {
            return Err("无法生成唯一标题".to_string());
        }
    }
}

fn note_title_taken(
    conn: &Connection,
    folder_id: &str,
    title: &str,
    exclude_note_id: Option<&str>,
) -> Result<bool, String> {
    let t = title.trim();
    if t.is_empty() {
        return Ok(true);
    }
    let c: i64 = match exclude_note_id {
        None => conn.query_row(
            "SELECT COUNT(*) FROM notes WHERE folder_id = ?1 AND TRIM(title) = ?2 AND deleted_at IS NULL",
            params![folder_id, t],
            |r| r.get(0),
        ),
        Some(ex) => conn.query_row(
            "SELECT COUNT(*) FROM notes WHERE folder_id = ?1 AND TRIM(title) = ?2 AND id != ?3 AND deleted_at IS NULL",
            params![folder_id, t, ex],
            |r| r.get(0),
        ),
    }
    .map_err(|e| e.to_string())?;
    Ok(c > 0)
}

fn apply_sort_orders_tx(
    tx: &rusqlite::Transaction<'_>,
    folder_id: &str,
    ordered_ids: &[String],
) -> Result<(), String> {
    for (i, nid) in ordered_ids.iter().enumerate() {
        let n = tx
            .execute(
                "UPDATE notes SET sort_order = ?1 WHERE id = ?2 AND folder_id = ?3",
                params![i as i64, nid, folder_id],
            )
            .map_err(|e| e.to_string())?;
        if n != 1 {
            return Err("笔记与文件夹不一致".to_string());
        }
    }
    Ok(())
}

pub fn place_note(
    conn: &mut Connection,
    note_id: String,
    target_folder_id: String,
    before_note_id: Option<String>,
) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let old_folder_id: String = tx
        .query_row(
            "SELECT folder_id FROM notes WHERE id = ?1 AND deleted_at IS NULL",
            params![note_id],
            |r| r.get(0),
        )
        .map_err(|_| "笔记不存在或已在废纸篓中".to_string())?;
    let title: String = tx
        .query_row(
            "SELECT title FROM notes WHERE id = ?1 AND deleted_at IS NULL",
            params![note_id],
            |r| r.get(0),
        )
        .map_err(|_| "笔记不存在或已在废纸篓中".to_string())?;

    let tgt_ok: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM folders WHERE id = ?1 AND deleted_at IS NULL",
            params![target_folder_id.clone()],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if tgt_ok == 0 {
        return Err("目标文件夹不存在或已在废纸篓中".to_string());
    }

    if old_folder_id != target_folder_id {
        let dup: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM notes WHERE folder_id = ?1 AND TRIM(title) = ?2 AND deleted_at IS NULL",
                params![target_folder_id, title.trim()],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if dup > 0 {
            return Err("该文件夹中已有同名笔记".to_string());
        }
    }

    let mut stmt = tx
        .prepare(
            "SELECT id FROM notes WHERE folder_id = ?1 AND deleted_at IS NULL ORDER BY sort_order ASC, updated_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let mut ids: Vec<String> = stmt
        .query_map(params![target_folder_id.clone()], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);

    ids.retain(|x| x != &note_id);
    let insert_at = match &before_note_id {
        None => ids.len(),
        Some(b) => ids
            .iter()
            .position(|x| x == b)
            .ok_or_else(|| "无效的插入位置".to_string())?,
    };
    ids.insert(insert_at, note_id.clone());

    if old_folder_id != target_folder_id {
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "UPDATE notes SET folder_id = ?1, updated_at = ?2 WHERE id = ?3 AND deleted_at IS NULL",
            params![target_folder_id.clone(), now, note_id],
        )
        .map_err(|e| e.to_string())?;
    }

    apply_sort_orders_tx(&tx, &target_folder_id, &ids)?;

    if old_folder_id != target_folder_id {
        let mut stmt = tx
            .prepare(
                "SELECT id FROM notes WHERE folder_id = ?1 AND deleted_at IS NULL ORDER BY sort_order ASC, updated_at DESC",
            )
            .map_err(|e| e.to_string())?;
        let old_ids: Vec<String> = stmt
            .query_map(params![old_folder_id.clone()], |r| r.get(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        drop(stmt);
        apply_sort_orders_tx(&tx, &old_folder_id, &old_ids)?;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_folders(conn: &Connection) -> Result<Vec<FolderRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, parent_id, name, sort_order, created_at FROM folders WHERE parent_id IS NULL AND deleted_at IS NULL ORDER BY sort_order, name",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(FolderRow {
                id: r.get(0)?,
                parent_id: r.get(1)?,
                name: r.get(2)?,
                sort_order: r.get(3)?,
                created_at: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub fn create_folder(conn: &mut Connection, name: String) -> Result<FolderRow, String> {
    let trimmed = name.trim().to_string();
    if trimmed.is_empty() {
        return Err("文件夹名称不能为空".to_string());
    }
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    if folder_name_taken(&*tx, None, &trimmed, None)? {
        return Err("已存在同名文件夹".to_string());
    }
    let parent_id: Option<String> = None;
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let sort_order: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM folders WHERE parent_id IS NULL AND deleted_at IS NULL",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO folders (id, parent_id, name, sort_order, created_at) VALUES (?1, NULL, ?2, ?3, ?4)",
        params![id, trimmed.clone(), sort_order, now],
    )
    .map_err(|e| {
        let s = e.to_string();
        if s.contains("UNIQUE constraint failed") || s.contains("unique constraint") {
            "已存在同名文件夹".to_string()
        } else {
            s
        }
    })?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(FolderRow {
        id,
        parent_id,
        name: trimmed,
        sort_order,
        created_at: now,
    })
}

pub fn rename_folder(conn: &Connection, id: String, name: String) -> Result<(), String> {
    let parent_id: Option<String> = conn
        .query_row(
            "SELECT parent_id FROM folders WHERE id = ?1 AND deleted_at IS NULL",
            params![id.clone()],
            |r| r.get(0),
        )
        .map_err(|_| "文件夹不存在或已在废纸篓中".to_string())?;
    if folder_name_taken(
        conn,
        parent_id.as_deref(),
        &name,
        Some(id.as_str()),
    )? {
        return Err("已存在同名文件夹".to_string());
    }
    conn.execute(
        "UPDATE folders SET name = ?1 WHERE id = ?2",
        params![name.trim(), id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_folder(conn: &mut Connection, id: String) -> Result<(), String> {
    let n = conn
        .query_row(
            "SELECT COUNT(*) FROM folders WHERE id = ?1 AND parent_id IS NULL AND deleted_at IS NULL",
            params![id.clone()],
            |r| r.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("文件夹不存在或已在废纸篓中".to_string());
    }
    let now = Utc::now().to_rfc3339();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE folders SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![now, id.clone()],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE notes SET deleted_at = ?1 WHERE folder_id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_notes(conn: &Connection, folder_id: String) -> Result<Vec<NoteRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, folder_id, title, preview, sort_order, created_at, updated_at, is_favorite, body_text_units FROM notes WHERE folder_id = ?1 AND deleted_at IS NULL ORDER BY sort_order ASC, updated_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![folder_id], |r| {
            Ok(NoteRow {
                id: r.get(0)?,
                folder_id: r.get(1)?,
                title: r.get(2)?,
                preview: r.get(3)?,
                sort_order: r.get(4)?,
                created_at: r.get(5)?,
                updated_at: r.get(6)?,
                is_favorite: r.get::<_, i64>(7)? != 0,
                body_text_units: r.get(8)?,
                content_html: None,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub fn list_all_notes(conn: &Connection) -> Result<Vec<NoteRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, folder_id, title, preview, sort_order, created_at, updated_at, is_favorite, body_text_units FROM notes WHERE deleted_at IS NULL ORDER BY folder_id, sort_order ASC, updated_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(NoteRow {
                id: r.get(0)?,
                folder_id: r.get(1)?,
                title: r.get(2)?,
                preview: r.get(3)?,
                sort_order: r.get(4)?,
                created_at: r.get(5)?,
                updated_at: r.get(6)?,
                is_favorite: r.get::<_, i64>(7)? != 0,
                body_text_units: r.get(8)?,
                content_html: None,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub fn create_note(conn: &Connection, folder_id: String, title: String) -> Result<NoteRow, String> {
    if folder_id.trim().is_empty() {
        return Err("请选择有效文件夹".to_string());
    }
    let ok: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM folders WHERE id = ?1 AND deleted_at IS NULL",
            params![folder_id.clone()],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if ok == 0 {
        return Err("文件夹不存在或已在废纸篓中".to_string());
    }
    let t = alloc_unique_note_title(conn, &folder_id, &title)?;
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let sort_order: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM notes WHERE folder_id = ?1 AND deleted_at IS NULL",
            params![folder_id.clone()],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO notes (id, folder_id, title, preview, sort_order, created_at, updated_at, body_text_units) VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, 0)",
        params![id, folder_id, t.clone(), sort_order, now, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(NoteRow {
        id,
        folder_id,
        title: t,
        preview: None,
        sort_order,
        created_at: now.clone(),
        updated_at: now,
        is_favorite: false,
        body_text_units: 0,
        content_html: None,
    })
}

pub fn set_note_favorite(conn: &Connection, id: String, favorite: bool) -> Result<NoteRow, String> {
    let fav: i64 = if favorite { 1 } else { 0 };
    let n = conn
        .execute(
            "UPDATE notes SET is_favorite = ?1 WHERE id = ?2 AND deleted_at IS NULL",
            params![fav, id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("笔记不存在或已在废纸篓中".to_string());
    }
    conn.query_row(
        "SELECT id, folder_id, title, preview, sort_order, created_at, updated_at, is_favorite, body_text_units FROM notes WHERE id = ?1 AND deleted_at IS NULL",
        params![id],
        |r| {
            Ok(NoteRow {
                id: r.get(0)?,
                folder_id: r.get(1)?,
                title: r.get(2)?,
                preview: r.get(3)?,
                sort_order: r.get(4)?,
                created_at: r.get(5)?,
                updated_at: r.get(6)?,
                is_favorite: r.get::<_, i64>(7)? != 0,
                body_text_units: r.get(8)?,
                content_html: None,
            })
        },
    )
    .map_err(|e| e.to_string())
}

pub fn rename_note(conn: &Connection, id: String, title: String) -> Result<(), String> {
    let folder_id: String = conn
        .query_row(
            "SELECT folder_id FROM notes WHERE id = ?1 AND deleted_at IS NULL",
            params![id.clone()],
            |r| r.get(0),
        )
        .map_err(|_| "笔记不存在或已在废纸篓中".to_string())?;
    if note_title_taken(conn, &folder_id, &title, Some(id.as_str()))? {
        return Err("该文件夹中已有同名笔记".to_string());
    }
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE notes SET title = ?1, updated_at = ?2 WHERE id = ?3",
        params![title.trim(), now, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_note_content(conn: &Connection, note_id: &str) -> Result<Option<String>, String> {
    let row: Option<Option<String>> = conn
        .query_row(
            "SELECT content_html FROM notes WHERE id = ?1 AND deleted_at IS NULL",
            params![note_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(match row.flatten() {
        Some(s) if !s.trim().is_empty() => Some(s),
        _ => None,
    })
}

pub fn set_note_content(conn: &Connection, note_id: String, content_html: String) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    let n = conn
        .execute(
            "UPDATE notes SET content_html = ?1, updated_at = ?2 WHERE id = ?3 AND deleted_at IS NULL",
            params![content_html, now, note_id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("笔记不存在或已在废纸篓中".to_string());
    }
    refresh_note_preview(conn, &note_id)?;
    Ok(())
}

pub fn delete_note(conn: &Connection, id: String) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    let n = conn
        .execute(
            "UPDATE notes SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
            params![now, id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("笔记不存在或已在废纸篓中".to_string());
    }
    Ok(())
}

pub fn list_trash_items(conn: &Connection) -> Result<Vec<TrashItemRow>, String> {
    purge_expired_trash(conn)?;

    let mut stmt = conn
        .prepare(
            "SELECT id, name, deleted_at FROM folders WHERE parent_id IS NULL AND deleted_at IS NOT NULL",
        )
        .map_err(|e| e.to_string())?;
    let folder_rows: Vec<(String, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);

    let mut parts: Vec<(String, String, String, Option<i64>, u8)> = Vec::new();
    for (id, name, da) in folder_rows {
        let c: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM notes WHERE folder_id = ?1 AND deleted_at IS NOT NULL",
                params![&id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        parts.push((id, name, da, Some(c), 0u8));
    }

    let mut stmt = conn
        .prepare(
            "SELECT n.id, n.title, n.deleted_at FROM notes n INNER JOIN folders f ON n.folder_id = f.id WHERE n.deleted_at IS NOT NULL AND f.deleted_at IS NULL",
        )
        .map_err(|e| e.to_string())?;
    let note_rows: Vec<(String, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);

    for (id, title, da) in note_rows {
        parts.push((id, title, da, None, 1u8));
    }

    parts.sort_by(|a, b| b.2.cmp(&a.2));

    let mut out = Vec::with_capacity(parts.len());
    for (id, title, da, nc, kind) in parts {
        out.push(TrashItemRow {
            kind: if kind == 0 {
                "folder".to_string()
            } else {
                "note".to_string()
            },
            id,
            title,
            deleted_at: da,
            note_count: nc,
        });
    }
    Ok(out)
}

pub fn restore_trash_folder(conn: &mut Connection, id: String) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let n = tx
        .execute(
            "UPDATE folders SET deleted_at = NULL WHERE id = ?1 AND parent_id IS NULL AND deleted_at IS NOT NULL",
            params![id.clone()],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("文件夹不存在或未被移入废纸篓".to_string());
    }
    tx.execute(
        "UPDATE notes SET deleted_at = NULL WHERE folder_id = ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn restore_trash_note(conn: &Connection, id: String) -> Result<(), String> {
    let n = conn
        .execute(
            "UPDATE notes SET deleted_at = NULL WHERE id = ?1 AND deleted_at IS NOT NULL AND EXISTS (SELECT 1 FROM folders f WHERE f.id = notes.folder_id AND f.deleted_at IS NULL)",
            params![id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("无法恢复（笔记不存在或所属文件夹仍在废纸篓中）".to_string());
    }
    Ok(())
}

fn refresh_note_preview(conn: &Connection, note_id: &str) -> Result<(), String> {
    let html_opt: Option<String> = conn
        .query_row(
            "SELECT content_html FROM notes WHERE id = ?1",
            params![note_id],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .flatten();

    let now = Utc::now().to_rfc3339();

    let (preview, units) = if let Some(ref h) = html_opt.as_ref().filter(|s| !s.trim().is_empty()) {
        let plain = strip_html_tags(h);
        let line = plain.lines().next().unwrap_or("").trim();
        let p = if line.is_empty() {
            None
        } else {
            Some(line.chars().take(120).collect::<String>())
        };
        (p, count_stat_text_from_html(h))
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT body FROM entries WHERE note_id = ?1 ORDER BY sort_order ASC, created_at ASC",
            )
            .map_err(|e| e.to_string())?;
        let bodies: Vec<String> = stmt
            .query_map(params![note_id], |r| r.get(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        drop(stmt);
        let concat = bodies.join("\n");
        let line = concat.lines().next().unwrap_or("").trim();
        let p = if line.is_empty() {
            None
        } else {
            Some(line.chars().take(120).collect::<String>())
        };
        (p, count_stat_text_from_plain(&concat))
    };

    conn.execute(
        "UPDATE notes SET preview = ?1, body_text_units = ?2, updated_at = ?3 WHERE id = ?4",
        params![preview, units, now, note_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_entries(conn: &Connection, note_id: String) -> Result<Vec<EntryRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, note_id, title, body, sort_order, created_at FROM entries WHERE note_id = ?1 ORDER BY sort_order ASC, created_at ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![note_id], |r| {
            Ok(EntryRow {
                id: r.get(0)?,
                note_id: r.get(1)?,
                title: r.get(2)?,
                body: r.get(3)?,
                sort_order: r.get(4)?,
                created_at: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub fn add_entry(conn: &Connection, note_id: String, title: String, body: String) -> Result<EntryRow, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let sort_order: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM entries WHERE note_id = ?1",
            params![note_id.clone()],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO entries (id, note_id, title, body, sort_order, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, note_id.clone(), title, body, sort_order, now],
    )
    .map_err(|e| e.to_string())?;
    refresh_note_preview(conn, &note_id)?;
    Ok(EntryRow {
        id,
        note_id,
        title,
        body,
        sort_order,
        created_at: now,
    })
}

pub fn delete_entry(conn: &Connection, id: String, note_id: String) -> Result<(), String> {
    conn.execute("DELETE FROM entries WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    refresh_note_preview(conn, &note_id)?;
    Ok(())
}

pub fn export_snapshot(conn: &Connection) -> Result<Vec<u8>, String> {
    let folders = list_folders(conn)?;
    let mut notes = list_all_notes(conn)?;
    for n in &mut notes {
        n.content_html = get_note_content(conn, &n.id)?;
    }
    let mut entries = Vec::new();
    for n in &notes {
        let mut e = list_entries(conn, n.id.clone())?;
        entries.append(&mut e);
    }
    let payload = TbookPayload {
        format: "tbook".to_string(),
        version: 1,
        exported_at: Utc::now().to_rfc3339(),
        folders,
        notes,
        entries,
    };
    let json = serde_json::to_vec_pretty(&payload).map_err(|e| e.to_string())?;
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        use std::io::Write;
        use zip::write::FileOptions;
        let mut zip = zip::ZipWriter::new(&mut buf);
        let opts = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("data.json", opts)
            .map_err(|e| e.to_string())?;
        zip.write_all(&json).map_err(|e| e.to_string())?;
        zip.finish().map_err(|e| e.to_string())?;
    }
    Ok(buf.into_inner())
}

pub fn import_replace(conn: &Connection, data: &[u8]) -> Result<(), String> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|e| e.to_string())?;
    let mut json_bytes = Vec::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        if file.name() == "data.json" {
            std::io::Read::read_to_end(&mut file, &mut json_bytes).map_err(|e| e.to_string())?;
            break;
        }
    }
    if json_bytes.is_empty() {
        return Err("tbook 中未找到 data.json".to_string());
    }
    let v: serde_json::Value = serde_json::from_slice(&json_bytes).map_err(|e| e.to_string())?;
    if v.get("format").and_then(|x| x.as_str()) != Some("tbook") {
        return Err("不是有效的 tbook 文件（缺少 format: tbook）".to_string());
    }
    let folders: Vec<FolderImport> = serde_json::from_value(
        v.get("folders")
            .cloned()
            .ok_or_else(|| "缺少 folders".to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let notes: Vec<NoteImport> = serde_json::from_value(
        v.get("notes")
            .cloned()
            .ok_or_else(|| "缺少 notes".to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let entries: Vec<EntryImport> = serde_json::from_value(
        v.get("entries")
            .cloned()
            .ok_or_else(|| "缺少 entries".to_string())?,
    )
    .map_err(|e| e.to_string())?;

    let folder_ids: HashSet<String> = folders.iter().map(|f| f.id.clone()).collect();
    for n in &notes {
        if !folder_ids.contains(&n.folder_id) {
            return Err(format!("笔记 {} 引用了不存在的文件夹 {}", n.title, n.folder_id));
        }
    }
    let note_ids: HashSet<String> = notes.iter().map(|n| n.id.clone()).collect();
    for e in &entries {
        if !note_ids.contains(&e.note_id) {
            return Err(format!("条目引用了不存在的笔记 {}", e.note_id));
        }
    }

    conn.execute_batch("PRAGMA foreign_keys = OFF; DELETE FROM entries; DELETE FROM notes; DELETE FROM folders; PRAGMA foreign_keys = ON;")
        .map_err(|e| e.to_string())?;

    let mut pending: HashSet<String> = folders.iter().map(|f| f.id.clone()).collect();
    let by_id: HashMap<String, &FolderImport> = folders.iter().map(|f| (f.id.clone(), f)).collect();
    let mut inserted: HashSet<String> = HashSet::new();

    while !pending.is_empty() {
        let mut batch = Vec::new();
        for id in &pending {
            let f = by_id.get(id).unwrap();
            let ok = match &f.parent_id {
                None => true,
                Some(pid) => inserted.contains(pid),
            };
            if ok {
                batch.push(id.clone());
            }
        }
        if batch.is_empty() {
            return Err("文件夹层级存在循环或无效的 parent_id".to_string());
        }
        for id in &batch {
            let f = by_id.get(id).unwrap();
            conn.execute(
                "INSERT INTO folders (id, parent_id, name, sort_order, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![f.id, f.parent_id.clone(), f.name, f.sort_order, f.created_at],
            )
            .map_err(|e| e.to_string())?;
            inserted.insert(id.clone());
            pending.remove(id);
        }
    }

    for n in &notes {
        let fav: i64 = if n.is_favorite { 1 } else { 0 };
        let bu = if n.body_text_units > 0 {
            n.body_text_units
        } else if let Some(ref h) = n.content_html.as_ref().filter(|s| !s.trim().is_empty()) {
            count_stat_text_from_html(h)
        } else {
            0i64
        };
        conn.execute(
            "INSERT INTO notes (id, folder_id, title, preview, sort_order, created_at, updated_at, content_html, is_favorite, body_text_units) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                n.id,
                n.folder_id,
                n.title,
                n.preview.clone(),
                n.sort_order,
                n.created_at,
                n.updated_at,
                n.content_html.clone(),
                fav,
                bu,
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    for e in &entries {
        conn.execute(
            "INSERT INTO entries (id, note_id, title, body, sort_order, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                e.id,
                e.note_id,
                e.title,
                e.body,
                e.sort_order,
                e.created_at
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    normalize_two_level_after_import(conn)?;

    Ok(())
}
