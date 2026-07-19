//! kanmusu page IPC



use std::sync::Arc;



use tauri::{AppHandle, State};



use crate::kanmusu::{

    self, KanmusuCharacterBrief, KanmusuCharacterDetail, KanmusuLine, KanmusuMenuSkinsPayload,

    KanmusuSkinDetail, KanmusuSyncResult,

};

use crate::pet::{self, COMPANION_ENGINE_KANMUSU};

use crate::state::AppState;



#[tauri::command]

pub async fn kanmusu_list(st: State<'_, Arc<AppState>>) -> Result<Vec<KanmusuCharacterBrief>, String> {

    let data_dir = st.data_dir();

    kanmusu::ensure_seeded(data_dir)?;

    kanmusu::list_brief(data_dir)

}



#[tauri::command]

pub async fn kanmusu_get_detail(

    st: State<'_, Arc<AppState>>,

    character_id: String,

) -> Result<KanmusuCharacterDetail, String> {

    kanmusu::get_detail(st.data_dir(), &character_id)

}



#[tauri::command]

pub async fn kanmusu_update_character(

    st: State<'_, Arc<AppState>>,

    character_id: String,

    name: Option<String>,

    description: Option<String>,

) -> Result<KanmusuCharacterDetail, String> {

    kanmusu::update_character(st.data_dir(), &character_id, name, description)

}



#[tauri::command]

pub async fn kanmusu_update_skin(

    st: State<'_, Arc<AppState>>,

    character_id: String,

    skin_id: String,

    name: Option<String>,

    lines: Option<Vec<KanmusuLine>>,

) -> Result<KanmusuSkinDetail, String> {

    kanmusu::update_skin(st.data_dir(), &character_id, &skin_id, name, lines)

}



#[tauri::command]

pub async fn kanmusu_sync_from_unpacked(

    st: State<'_, Arc<AppState>>,

) -> Result<KanmusuSyncResult, String> {

    let mut result = kanmusu::sync_from_unpacked(st.data_dir())?;
    // 锁外挂到人物皮肤，避免与 kanmusu sync 嵌套死锁
    let _ = crate::character::attach_kanmusu_after_sync(st.data_dir());
    result.message = format!(
        "{}；已更新人物皮肤舰娘绑定",
        result.message.trim_end_matches('。')
    );
    Ok(result)

}



#[tauri::command]

pub async fn kanmusu_player_open(app: AppHandle) -> Result<(), String> {

    kanmusu::player_open(&app)

}



#[tauri::command]

pub async fn kanmusu_player_close(app: AppHandle) -> Result<(), String> {

    kanmusu::player_close(&app)

}



/// 独立预览窗加载舰娘（不顶替桌宠）
#[tauri::command]
pub async fn kanmusu_player_load(
    app: AppHandle,
    st: State<'_, Arc<AppState>>,
    character_id: String,
    skin_id: String,
) -> Result<(), String> {
    kanmusu::preview_open(&app, st.inner(), &character_id, &skin_id)
}



#[tauri::command]

pub async fn kanmusu_desktop_open(

    app: AppHandle,

    st: State<'_, Arc<AppState>>,

    character_id: String,

    skin_id: String,

) -> Result<(), String> {

    kanmusu::desktop_open(&app, st.inner(), &character_id, &skin_id)

}



#[tauri::command]

pub async fn kanmusu_player_consume_pending(

    app: AppHandle,

) -> Result<Option<kanmusu::KanmusuPlayerLoadPayload>, String> {

    Ok(kanmusu::consume_pending_player_load(&app))

}



#[tauri::command]

pub async fn kanmusu_read_model_asset(

    st: State<'_, Arc<AppState>>,

    model_dir: String,

    filename: String,

) -> Result<String, String> {

    kanmusu::read_model_asset_b64(st.data_dir(), &model_dir, &filename)

}



#[tauri::command]

pub async fn kanmusu_read_model_bundle(

    st: State<'_, Arc<AppState>>,

    model_dir: String,

    filenames: Vec<String>,

) -> Result<kanmusu::KanmusuModelAssetBundle, String> {

    kanmusu::read_model_asset_bundle(st.data_dir(), &model_dir, &filenames)

}



#[tauri::command]

pub async fn kanmusu_prime_model(

    st: State<'_, Arc<AppState>>,

    model_dir: String,

    model3_filename: String,

    priority_names: Option<Vec<String>>,

) -> Result<kanmusu::KanmusuPrimeModelPayload, String> {

    kanmusu::prime_model(

        st.data_dir(),

        &model_dir,

        &model3_filename,

        priority_names.as_deref().unwrap_or(&[]),

    )

}



#[tauri::command]

pub async fn pet_get_companion_engine(

    st: State<'_, Arc<AppState>>,

) -> Result<String, String> {

    let db = crate::db::lock_conn(&st.db)?;

    Ok(pet::get_companion_engine(&db))

}



#[tauri::command]

pub async fn kanmusu_menu_characters(

    st: State<'_, Arc<AppState>>,

) -> Result<Vec<KanmusuCharacterBrief>, String> {

    kanmusu::menu_list_brief(st.data_dir())

}



#[tauri::command]

pub async fn kanmusu_menu_skins(

    st: State<'_, Arc<AppState>>,

) -> Result<KanmusuMenuSkinsPayload, String> {

    let db = crate::db::lock_conn(&st.db)?;

    kanmusu::menu_skins_for(st.data_dir(), &db, None)

}



#[tauri::command]

pub async fn kanmusu_menu_skins_for(

    st: State<'_, Arc<AppState>>,

    character_id: String,

) -> Result<KanmusuMenuSkinsPayload, String> {

    let db = crate::db::lock_conn(&st.db)?;

    kanmusu::menu_skins_for(st.data_dir(), &db, Some(&character_id))

}



#[tauri::command]

pub async fn kanmusu_menu_switch_skin(

    app: AppHandle,

    st: State<'_, Arc<AppState>>,

    character_id: String,

    skin_id: String,

) -> Result<(), String> {

    let db = crate::db::lock_conn(&st.db)?;

    let engine = pet::get_companion_engine(&db);

    drop(db);

    if engine != COMPANION_ENGINE_KANMUSU {

        // Switching kanmusu skin forces kanmusu mode (direct replace).

    }

    kanmusu::menu_switch_skin(&app, st.inner(), &character_id, &skin_id)

}


