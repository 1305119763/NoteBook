mod db;

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
fn export_tbook(state: State<DbState>, path: String) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let bytes = db::export_snapshot(&conn)?;
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

#[tauri::command]
fn import_tbook(state: State<DbState>, path: String) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    db::import_replace(&conn, &bytes)
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
