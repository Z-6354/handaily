//! 从本地 BWIKI SQLite 批量导入舰娘人设（走与 Wiki 导入相同的 AI 流水线）

use std::sync::Mutex;

use xiaohan_daily_lib::persona::import_reference::{
    import_from_reference, import_persona_from_blhx_reference_fast, load_blhx_ship_reference,
    resolve_blhx_db_path, ImportReferenceContext, ImportReferenceProgress,
};
use xiaohan_daily_lib::live2d::VaultState;

use xiaohan_daily_lib::data_layout;

fn open_handaily_db(data_dir: &std::path::Path) -> Result<rusqlite::Connection, String> {
    std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    xiaohan_daily_lib::db::open_and_migrate(&data_layout::db_path(data_dir)).map_err(|e| e.to_string())
}

fn list_all_catalog_titles(blhx_path: &std::path::Path) -> Result<Vec<String>, String> {
    let conn = rusqlite::Connection::open(blhx_path).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT wiki_title FROM catalog ORDER BY display_name")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<String>, _>>()
        .map_err(|e| e.to_string())
}

fn list_all_ship_titles(blhx_path: &std::path::Path) -> Result<Vec<String>, String> {
    let conn = rusqlite::Connection::open(blhx_path).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT wiki_title FROM ships WHERE length(trim(persona_reference)) > 0 ORDER BY display_name",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<String>, _>>()
        .map_err(|e| e.to_string())
}

fn existing_persona_names(data_dir: &std::path::Path) -> std::collections::HashSet<String> {
    let manifest = xiaohan_daily_lib::persona::load_manifest(data_dir);
    manifest.personas.iter().map(|p| p.name.clone()).collect()
}

fn print_usage() {
    eprintln!(
        r#"用法:
  blhx_import 舰娘1,舰娘2,...
  blhx_import --all [--limit N] [--skip-existing]
  blhx_import --all --reference-only [--limit N] [--skip-existing]
  blhx_import --sync-characters

示例:
  blhx_import 欧根亲王,贝尔法斯特,胡德
  blhx_import --all --reference-only --skip-existing   # 快速批量，不调用 AI
  blhx_import --all --limit 20 --skip-existing         # AI 流水线，较慢
"#
    );
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("blhx_import 失败: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        print_usage();
        return if args.is_empty() {
            Err("缺少参数".into())
        } else {
            Ok(())
        };
    }

    let mut all = false;
    let mut sync_only = false;
    let mut reference_only = false;
    let mut limit: Option<usize> = None;
    let mut skip_existing = false;
    let mut titles: Vec<String> = vec![];

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--all" => all = true,
            "--sync-characters" => sync_only = true,
            "--reference-only" => reference_only = true,
            "--limit" => {
                i += 1;
                limit = Some(
                    args.get(i)
                        .ok_or("--limit 需要数字")?
                        .parse()
                        .map_err(|_| "limit 须为正整数".to_string())?,
                );
            }
            "--skip-existing" => skip_existing = true,
            other if other.starts_with('-') => return Err(format!("未知参数: {other}")),
            other => titles.push(other.to_string()),
        }
        i += 1;
    }

    let blhx_path = if sync_only {
        None
    } else {
        Some(resolve_blhx_db_path()?)
    };
    let data_dir = data_layout::handaily_data_dir()?;
    let _ = xiaohan_daily_lib::prompts::seed_user_prompts(&data_dir);
    let _ = xiaohan_daily_lib::persona::seed_user_personas(&data_dir);
    let _ = xiaohan_daily_lib::character::seed_user_characters(&data_dir);

    if sync_only {
        let (synced, _, _) =
            xiaohan_daily_lib::character::sync_character_manifest_from_personas(&data_dir)?;
        println!(
            "人物 manifest 已从 persona 同步（{} 变更）\n数据目录: {}",
            if synced { "有" } else { "无" },
            data_dir.display()
        );
        return Ok(());
    }

    if all {
        let blhx = blhx_path.as_ref().unwrap();
        titles = if reference_only {
            list_all_ship_titles(blhx)?
        } else {
            list_all_catalog_titles(blhx)?
        };
        if skip_existing {
            let existing = existing_persona_names(&data_dir);
            titles.retain(|t| !existing.contains(t));
        }
        if let Some(n) = limit {
            titles.truncate(n);
        }
    } else if titles.len() == 1 && titles[0].contains(',') {
        titles = titles[0]
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
    }

    if titles.is_empty() {
        return Err("舰娘名称列表为空".into());
    }

    let blhx_path = blhx_path.unwrap();

    if reference_only {
        println!(
            "HANDAILY: {}\nBWIKI: {}\n快速导入 {} 个角色（无 AI）\n",
            data_dir.display(),
            blhx_path.display(),
            titles.len()
        );
        let mut ok = 0;
        let mut skipped = 0;
        let mut failed = 0;
        for (i, title) in titles.iter().enumerate() {
            match import_persona_from_blhx_reference_fast(
                &data_dir,
                &blhx_path,
                title,
                skip_existing,
            ) {
                Ok(msg) => {
                    if msg.starts_with("跳过") {
                        skipped += 1;
                    } else {
                        ok += 1;
                    }
                    if (i + 1) % 50 == 0 || i + 1 == titles.len() {
                        println!("[{}/{}] {msg}", i + 1, titles.len());
                    }
                }
                Err(e) => {
                    failed += 1;
                    eprintln!("[{}/{}] {title}: {e}", i + 1, titles.len());
                }
            }
        }
        let _ = xiaohan_daily_lib::character::sync_character_manifest_from_personas(&data_dir)?;
        println!("\n完成: 导入 {ok}，跳过 {skipped}，失败 {failed}");
        return Ok(());
    }

    let db = open_handaily_db(&data_dir)?;
    let db = Mutex::new(db);
    let vault = VaultState::new();
    {
        let guard = db.lock().map_err(|e| e.to_string())?;
        vault.load_config(&guard)?;
    }

    println!(
        "HANDAILY 数据目录: {}\nBWIKI 数据库: {}\n待导入 {} 个角色（AI 流水线）\n",
        data_dir.display(),
        blhx_path.display(),
        titles.len()
    );

    for (i, title) in titles.iter().enumerate() {
        println!("[{}/{}] 导入 {title}…", i + 1, titles.len());
        let (display_name, text) = load_blhx_ship_reference(&blhx_path, title)?;
        let persona_id = xiaohan_daily_lib::persona::suggest_persona_id(&data_dir, &display_name)?;
        let ctx = ImportReferenceContext {
            data_dir: &data_dir,
            db: &db,
            vault: &vault,
            app: None,
        };
        let result = import_from_reference(
            &ctx,
            None,
            Some(persona_id.as_str()),
            Some(display_name.as_str()),
            Some("碧蓝航线 BWIKI"),
            &text,
            ImportReferenceProgress::wiki_pipeline(),
            true,
            false,
        )
        .await?;
        println!("  ✓ {}", result.message);
    }

    let (synced, _, _) =
        xiaohan_daily_lib::character::sync_character_manifest_from_personas(&data_dir)?;
    println!(
        "\n全部完成。人物 manifest {}。",
        if synced { "已同步" } else { "无变更" }
    );
    Ok(())
}
