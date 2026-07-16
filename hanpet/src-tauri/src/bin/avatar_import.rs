//! 从 BWIKI 图鉴同步人物标签（头像已改为 Wiki URL，不再下载）

use std::env;

use xiaohan_daily_lib::character::avatar::run_avatar_import_default;
use xiaohan_daily_lib::data_layout;
#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("avatar_import 失败: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    let mut limit = 50usize;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--limit" => {
                i += 1;
                limit = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(50);
            }
            "--all" => limit = usize::MAX,
            "--help" | "-h" => {
                println!(
                    "用法: avatar_import [--limit N] [--all]\n\
                     从 mcp/blhx-wiki 图鉴同步阵营/舰种/稀有度标签到 manifest\n\
                     头像直接使用 Wiki URL，不再下载到本地"
                );
                return Ok(());
            }
            "--force" | "--no-tags" => {}
            other => return Err(format!("未知参数: {other}")),
        }
        i += 1;
    }

    let data_dir = data_layout::handaily_data_dir()?;
    let result = run_avatar_import_default(&data_dir, limit, true, true).await?;
    println!("{}", result.message);
    Ok(())
}
