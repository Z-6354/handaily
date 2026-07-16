//! 从本地 AppData 导出角色资源 zip 包

use std::env;
use std::path::PathBuf;

use xiaohan_daily_lib::data_layout;
use xiaohan_daily_lib::roster_pack;

fn default_output_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("release")
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 || args[1] != "export" {
        eprintln!(
            "用法: roster_pack export [--output DIR]\n\
             \n\
             生成 zip 包：\n\
               - 模型-完整角色包\n\
               - 模型-皇家/白鹰/重樱/铁血阵营角色包\n\
               - 模型-其他角色包（未分类及维希教廷、北方联合等）\n\
               - 模型-柴郡角色包\n\
             \n\
             默认输出到仓库 release/（与安装包同目录，便于上传）"
        );
        std::process::exit(1);
    }

    let mut output = default_output_dir();
    let mut i = 2;
    while i < args.len() {
        if args[i] == "--output" {
            i += 1;
            let Some(path) = args.get(i) else {
                eprintln!("错误: --output 需要路径");
                std::process::exit(1);
            };
            output = PathBuf::from(path);
        }
        i += 1;
    }

    let data_dir = data_layout::handaily_data_dir().expect("无法定位 AppData 数据目录");
    println!("数据目录: {}", data_dir.display());
    println!("输出目录: {}", output.display());

    let summary = roster_pack::export_all_packs(&data_dir, &output).expect("导出失败");
    println!("\n导出完成，共 {} 个包：", summary.packs.len());
    for pack in &summary.packs {
        let mb = pack.size_bytes as f64 / 1024.0 / 1024.0;
        println!(
            "  {} — {} 角色 / {} 模型 / {:.1} MB",
            pack.file_name, pack.character_count, pack.model_count, mb
        );
    }
}
