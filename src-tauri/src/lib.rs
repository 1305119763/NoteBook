mod db;
mod media;
mod present;
mod settings;

use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{Emitter, Manager, State};

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
fn set_folder_color(state: State<DbState>, id: String, color: String) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    db::set_folder_color(&conn, id, color)
}

#[tauri::command]
fn place_folder(
    state: State<DbState>,
    folder_id: String,
    before_folder_id: Option<String>,
) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    db::place_folder(&conn, folder_id, before_folder_id)
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
fn set_note_pinned(
    state: State<DbState>,
    note_id: String,
    pinned: bool,
) -> Result<db::NoteRow, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    db::set_note_pinned(&conn, note_id, pinned)
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
    let mut payload = {
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        db::load_tbook_payload(&conn)?
    };
    let keys = db::collect_payload_media_keys(&payload);
    let files = media::read_export_media(&keys)?;
    let renames = media::export_note_media(Path::new(&path), &keys)?;
    db::apply_media_renames(&mut payload, &renames);
    let packed: Vec<(String, Vec<u8>)> = files
        .into_iter()
        .map(|(key, bytes)| (renames.get(&key).cloned().unwrap_or(key), bytes))
        .collect();
    let bytes = db::tbook_payload_bytes(&payload, &packed)?;
    let tmp = format!("{path}.writing");
    std::fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn import_tbook(state: State<DbState>, path: String) -> Result<(), String> {
    use std::path::Path;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let mut plan = db::plan_import(&bytes)?;
    let sidecar = media::read_sidecar_media(Path::new(&path))?;
    for (key, file) in sidecar {
        plan.media.entry(key).or_insert(file);
    }
    let missing: Vec<&String> = plan
        .media_keys
        .iter()
        .filter(|key| !plan.media.contains_key(*key) && !media::media_file_exists(key))
        .collect();
    if !missing.is_empty() {
        let names = missing.iter().map(|k| k.as_str()).collect::<Vec<_>>().join("、");
        return Err(format!(
            "导入包缺少这些图片或视频，已取消导入，当前笔记未改动：{names}"
        ));
    }
    {
        let mut conn = state.0.lock().map_err(|e| e.to_string())?;
        db::import_replace(&mut conn, &plan)?;
    }
    media::install_media_files(&plan.media).map_err(|e| {
        format!("笔记已导入，但媒体文件写入失败，请再次导入同一文件：{e}")
    })?;
    Ok(())
}

#[tauri::command]
fn resolve_media_keys(keys: Vec<String>) -> Result<Vec<media::ResolvedMedia>, String> {
    media::resolve_media_keys(&keys)
}

#[tauri::command]
fn record_note_revision(
    state: State<DbState>,
    note_id: String,
    title: String,
    content_html: String,
) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    db::record_note_revision(&conn, note_id, title, content_html)
}

#[tauri::command]
fn list_note_revisions(
    state: State<DbState>,
    note_id: String,
) -> Result<Vec<db::RevisionMeta>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    db::list_note_revisions(&conn, note_id)
}

#[tauri::command]
fn get_note_revision(
    state: State<DbState>,
    revision_id: String,
) -> Result<Option<db::RevisionDetail>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    db::get_note_revision(&conn, revision_id)
}

#[tauri::command]
fn start_present(
    note_id: String,
    title: String,
    html: String,
    max_editors: Option<u32>,
) -> Result<present::PresentStatus, String> {
    present::start(note_id, title, html, max_editors.unwrap_or(3))
}

#[tauri::command]
fn update_present(title: String, html: String) -> Result<(), String> {
    present::update(title, html)
}

#[tauri::command]
fn stop_present() -> Result<(), String> {
    present::stop();
    Ok(())
}

#[tauri::command]
fn set_present_max_editors(max_editors: u32) -> Result<(), String> {
    present::set_max_editors(max_editors)
}

#[tauri::command]
fn present_decide(ip: String, allow: bool) -> Result<(), String> {
    present::decide(ip, allow)
}

#[tauri::command]
fn present_revoke(ip: Option<String>) -> Result<(), String> {
    present::revoke(ip)
}

#[tauri::command]
fn set_present_nickname(nickname: String) {
    present::set_nickname(nickname);
}

#[tauri::command]
fn get_settings() -> Result<settings::SettingsFile, String> {
    settings::load()
}

#[tauri::command]
fn save_settings(settings: settings::AppSettings) -> Result<settings::SettingsFile, String> {
    settings::save(settings)
}

#[tauri::command]
fn present_status() -> Result<Option<present::PresentStatus>, String> {
    Ok(present::status())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
fn menu_item(
    app: &tauri::AppHandle,
    id: &str,
    label: &str,
    accel: Option<&str>,
) -> Result<MenuItem<tauri::Wry>, Box<dyn std::error::Error>> {
    Ok(MenuItem::with_id(app, id, label, true, accel)?)
}

fn install_shell(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let h = app.handle();
    let about = menu_item(h, "about", "关于 NoteBook", None)?;
    let settings = menu_item(h, "settings", "设置…", Some("CmdOrCtrl+,"))?;
    let hide = PredefinedMenuItem::hide(h, Some("隐藏 NoteBook"))?;
    let quit = PredefinedMenuItem::quit(h, Some("退出 NoteBook"))?;
    let new_note = menu_item(h, "new-note", "新建笔记", Some("CmdOrCtrl+N"))?;
    let new_folder = menu_item(h, "new-folder", "新建文件夹", Some("CmdOrCtrl+Shift+N"))?;
    let import_item = menu_item(h, "import", "导入 .tbook", None)?;
    let export_item = menu_item(h, "export", "导出 .tbook", None)?;
    let undo = PredefinedMenuItem::undo(h, Some("撤销"))?;
    let redo = PredefinedMenuItem::redo(h, Some("重做"))?;
    let cut = PredefinedMenuItem::cut(h, Some("剪切"))?;
    let copy = PredefinedMenuItem::copy(h, Some("复制"))?;
    let paste = PredefinedMenuItem::paste(h, Some("粘贴"))?;
    let select_all = PredefinedMenuItem::select_all(h, Some("全选"))?;
    let minimize = PredefinedMenuItem::minimize(h, Some("最小化"))?;
    let maximize = PredefinedMenuItem::maximize(h, Some("缩放"))?;
    let close = PredefinedMenuItem::close_window(h, Some("关闭窗口"))?;
    let help_about = menu_item(h, "about-help", "关于与开源地址", None)?;

    let app_menu = Submenu::with_items(
        h,
        "NoteBook",
        true,
        &[
            &about,
            &settings,
            &PredefinedMenuItem::separator(h)?,
            &hide,
            &quit,
        ],
    )?;
    let file_menu = Submenu::with_items(
        h,
        "文件",
        true,
        &[
            &new_note,
            &new_folder,
            &PredefinedMenuItem::separator(h)?,
            &import_item,
            &export_item,
        ],
    )?;
    let edit_menu = Submenu::with_items(
        h,
        "编辑",
        true,
        &[
            &undo,
            &redo,
            &PredefinedMenuItem::separator(h)?,
            &cut,
            &copy,
            &paste,
            &select_all,
        ],
    )?;
    let window_menu = Submenu::with_items(
        h,
        "窗口",
        true,
        &[&minimize, &maximize, &close],
    )?;
    let help_menu = Submenu::with_items(h, "帮助", true, &[&help_about])?;
    let menu = Menu::with_items(h, &[&app_menu, &file_menu, &edit_menu, &window_menu, &help_menu])?;
    app.set_menu(menu)?;
    app.on_menu_event(|app, event| {
        let _ = app.emit("nb-menu", event.id().as_ref().to_string());
    });

    let tray_settings = menu_item(h, "tray-settings", "设置…", None)?;
    let tray_about = menu_item(h, "tray-about", "关于 NoteBook", None)?;
    let tray_quit = PredefinedMenuItem::quit(h, Some("退出 NoteBook"))?;
    let tray_menu = Menu::with_items(
        h,
        &[
            &tray_settings,
            &tray_about,
            &PredefinedMenuItem::separator(h)?,
            &tray_quit,
        ],
    )?;
    let mut tray = tauri::tray::TrayIconBuilder::with_id("nb-tray")
        .menu(&tray_menu)
        .tooltip("NoteBook")
        .on_menu_event(|app, event| {
            let _ = app.emit("nb-menu", event.id().as_ref().to_string());
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let conn = db::open(&app.handle())?;
            app.manage(DbState(Mutex::new(conn)));
            install_shell(app)?;
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
            set_note_pinned,
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
            resolve_media_keys,
            record_note_revision,
            list_note_revisions,
            get_note_revision,
            start_present,
            update_present,
            stop_present,
            set_present_max_editors,
            present_decide,
            present_revoke,
            set_present_nickname,
            present_status,
            get_settings,
            save_settings,
            set_folder_color,
            place_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
