mod db;
mod media;

use std::sync::Mutex;
use tauri::{Manager, State};

pub struct DbState(Mutex<rusqlite::Connection>);

#[tauri::command]
fn list_folders(state: State<DbState>) -> Result<Vec<db::FolderRow>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    db::list_folders(&conn)
}

#[tauri::command]
fn create_folder(state: State<DbState>, name: String) -> Result<db::FolderRow, String> {
    let mut conn = state.0.lock().map_err(|e| e.to_string())?;
    db::create_folder(&mut conn, name)
}

#[tauri::command]
fn rename_folder(state: State<DbState>, id: String, name: String) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    db::rename_folder(&conn, id, name)
}

#[tauri::command]
fn delete_folder(state: State<DbState>, id: String) -> Result<(), String> {
    let mut conn = state.0.lock().map_err(|e| e.to_string())?;
    db::delete_folder(&mut conn, id)
}

#[tauri::command]
fn list_notes(state: State<DbState>, folder_id: String) -> Result<Vec<db::NoteRow>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    db::list_notes(&conn, folder_id)
}

#[tauri::command]
fn list_all_notes(state: State<DbState>) -> Result<Vec<db::NoteRow>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    db::list_all_notes(&conn)
}

#[tauri::command]
fn create_note(
    state: State<DbState>,
    folder_id: String,
    title: String,
) -> Result<db::NoteRow, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    db::create_note(&conn, folder_id, title)
}

#[tauri::command]
fn rename_note(state: State<DbState>, id: String, title: String) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    db::rename_note(&conn, id, title)
}

#[tauri::command]
fn set_note_favorite(
    state: State<DbState>,
    note_id: String,
    favorite: bool,
) -> Result<db::NoteRow, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    db::set_note_favorite(&conn, note_id, favorite)
}

#[tauri::command]
fn place_note(
    state: State<DbState>,
    note_id: String,
    target_folder_id: String,
    before_note_id: Option<String>,
) -> Result<(), String> {
    let mut conn = state.0.lock().map_err(|e| e.to_string())?;
    db::place_note(
        &mut conn,
        note_id,
        target_folder_id,
        before_note_id,
    )
}

#[tauri::command]
fn get_note_content(state: State<DbState>, note_id: String) -> Result<Option<String>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    db::get_note_content(&conn, &note_id)
}

#[tauri::command]
fn save_note_content(
    state: State<DbState>,
    note_id: String,
    content_html: String,
) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    db::set_note_content(&conn, note_id, content_html)
}

#[tauri::command]
fn delete_note(state: State<DbState>, id: String) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    db::delete_note(&conn, id)
}

#[tauri::command]
fn list_trash_items(state: State<DbState>) -> Result<Vec<db::TrashItemRow>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    db::list_trash_items(&conn)
}

#[tauri::command]
fn restore_trash_folder(state: State<DbState>, id: String) -> Result<(), String> {
    let mut conn = state.0.lock().map_err(|e| e.to_string())?;
    db::restore_trash_folder(&mut conn, id)
}

#[tauri::command]
fn restore_trash_note(state: State<DbState>, id: String) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    db::restore_trash_note(&conn, id)
}

#[tauri::command]
fn list_entries(state: State<DbState>, note_id: String) -> Result<Vec<db::EntryRow>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    db::list_entries(&conn, note_id)
}

#[tauri::command]
fn add_entry(
    state: State<DbState>,
    note_id: String,
    title: String,
    body: String,
) -> Result<db::EntryRow, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    db::add_entry(&conn, note_id, title, body)
}

#[tauri::command]
fn delete_entry(state: State<DbState>, id: String, note_id: String) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    db::delete_entry(&conn, id, note_id)
}

#[tauri::command]
fn import_media_file(_state: State<DbState>, file_path: String) -> Result<media::ImportedMedia, String> {
    media::import_media_from_path(&file_path)
}

#[tauri::command]
fn import_media_bytes(_state: State<DbState>, data: Vec<u8>, extension: String) -> Result<media::ImportedMedia, String> {
    media::import_media_bytes(data, extension)
}

#[tauri::command]
fn export_tbook(state: State<DbState>, path: String) -> Result<(), String> {
    use std::path::Path;
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let bytes = db::export_snapshot(&conn)?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;

    // 收集所有笔记中的媒体引用，导出同名媒体文件夹
    let media_keys = media::collect_all_media_keys(&conn)?;
    if !media_keys.is_empty() {
        let tbook_path = Path::new(&path);
        let parent = tbook_path.parent().unwrap_or(Path::new("."));
        let stem = tbook_path.file_stem().unwrap_or_default().to_string_lossy();
        let media_export_dir = parent.join(format!("{}.media", stem));

        // 清理旧目录并重建
        if media_export_dir.exists() {
            std::fs::remove_dir_all(&media_export_dir).map_err(|e| e.to_string())?;
        }
        std::fs::create_dir_all(&media_export_dir).map_err(|e| e.to_string())?;

        // 复制媒体文件到导出目录
        let media_dir = media::media_dir()?;
        let mut bindings: Vec<media::MediaBinding> = Vec::new();
        for key in &media_keys {
            let src = media_dir.join(key);
            let dest = media_export_dir.join(key);
            if src.is_file() {
                std::fs::copy(&src, &dest).map_err(|e| e.to_string())?;
                bindings.push(media::MediaBinding {
                    storage_key: key.clone(),
                    note_ids: vec![], // 绑定信息已存在于 HTML 中，此处仅记录文件
                });
            }
        }

        // 写入绑定清单
        let manifest_path = media_export_dir.join("media_manifest.json");
        let manifest = media::MediaExportManifest {
            version: "1.0.0".to_string(),
            bindings,
        };
        let manifest_json = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
        std::fs::write(&manifest_path, manifest_json).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn import_tbook(state: State<DbState>, path: String) -> Result<(), String> {
    use std::path::Path;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    db::import_replace(&conn, &bytes)?;

    // 导入同名媒体文件夹
    let tbook_path = Path::new(&path);
    let parent = tbook_path.parent().unwrap_or(Path::new("."));
    let stem = tbook_path.file_stem().unwrap_or_default().to_string_lossy();
    let media_import_dir = parent.join(format!("{}.media", stem));

    if media_import_dir.exists() && media_import_dir.is_dir() {
        // 复制媒体文件到应用数据目录，但不绑定（已通过HTML引用绑定）
        for entry in std::fs::read_dir(&media_import_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_file() {
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    if file_name != "media_manifest.json" {
                        let dest = media::media_dir()?.join(file_name);
                        std::fs::copy(&path, &dest).map_err(|e| e.to_string())?;
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let conn = db::open(&app.handle())?;
            app.manage(DbState(Mutex::new(conn)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_folders,
            create_folder,
            rename_folder,
            delete_folder,
            list_notes,
            list_all_notes,
            create_note,
            rename_note,
            set_note_favorite,
            place_note,
            get_note_content,
            save_note_content,
            delete_note,
            list_trash_items,
            restore_trash_folder,
            restore_trash_note,
            list_entries,
            add_entry,
            delete_entry,
            export_tbook,
            import_tbook,
            import_media_file,
            import_media_bytes,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
