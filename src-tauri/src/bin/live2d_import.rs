//! 从 live2d 目录批量导入 Spine 模型并绑定到对应人物皮肤

use std::path::PathBuf;

use xiaohan_daily_lib::live2d_import::{
    handaily_data_dir, resolve_live2d_root, resolve_plan_path, run_live2d_import,
};

fn print_usage() {
    eprintln!(
        r#"用法:
  live2d_import --plan plan.json [--live2d-root PATH] [--dry-run] [--limit N] [--all]

  --all    按 --limit（默认 100）循环导入直至完成

环境变量:
  HANDAILY_DATA_DIR       小寒日报 data 目录
  HANDAILY_LIVE2D_PATH    live2d 根目录
  HANDAILY_LIVE2D_PLAN    导入计划 JSON 路径
"#
    );
}

fn main() {
    if let Err(e) = run() {
        eprintln!("live2d_import 失败: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        print_usage();
        return if args.is_empty() {
            Err("缺少 --plan".into())
        } else {
            Ok(())
        };
    }

    let mut plan_path: Option<PathBuf> = None;
    let mut live2d_root: Option<PathBuf> = None;
    let mut dry_run = false;
    let mut all = false;
    let mut limit = usize::MAX;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--plan" => {
                i += 1;
                plan_path = Some(PathBuf::from(args.get(i).ok_or("--plan 需要路径")?));
            }
            "--live2d-root" => {
                i += 1;
                live2d_root = Some(PathBuf::from(args.get(i).ok_or("--live2d-root 需要路径")?));
            }
            "--dry-run" => dry_run = true,
            "--all" => all = true,
            "--limit" => {
                i += 1;
                limit = args
                    .get(i)
                    .ok_or("--limit 需要数字")?
                    .parse()
                    .map_err(|_| "limit 须为正整数".to_string())?;
            }
            other => return Err(format!("未知参数: {other}")),
        }
        i += 1;
    }

    let data_dir = handaily_data_dir()?;
    let _ = xiaohan_daily_lib::character::seed_user_characters(&data_dir);
    let db = xiaohan_daily_lib::db::open_and_migrate(&data_dir.join("xiaohan.sqlite"))
        .map_err(|e| e.to_string())?;

    let plan = resolve_plan_path(plan_path.as_deref())?;
    let root = live2d_root.unwrap_or_else(resolve_live2d_root);

    println!(
        "HANDAILY: {}\n计划: {}\nlive2d: {}\n",
        data_dir.display(),
        plan.display(),
        root.display()
    );

    let batch_limit = if all {
        if limit == usize::MAX {
            100
        } else {
            limit
        }
    } else {
        limit
    };

    if all {
        loop {
            let result = run_live2d_import(&data_dir, &db, &plan, &root, batch_limit, dry_run)?;
            println!("{}", result.message);
            if result.remaining == 0 || result.processed == 0 {
                break;
            }
        }
    } else {
        let result = run_live2d_import(&data_dir, &db, &plan, &root, batch_limit, dry_run)?;
        println!("{}", result.message);
    }
    Ok(())
}
