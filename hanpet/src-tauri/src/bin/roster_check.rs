//! 诊断人物列表 / 详情加载

use std::time::Instant;

use xiaohan_daily_lib::character;
use xiaohan_daily_lib::data_layout;
use xiaohan_daily_lib::db;

fn main() {
    let data_dir = data_layout::handaily_data_dir()
        .expect("HANDAILY_DATA_DIR / APPDATA");
    let db_path = data_layout::db_path(&data_dir);
    let db = db::open_and_migrate(&db_path).expect("open db");

    let t0 = Instant::now();
    let page = character::list_characters_page(&data_dir, &db, 0, 48, None, false, &[]);
    println!("list page: total={} items={} {:.2?}", page.total, page.items.len(), t0.elapsed());

    if let Some(first) = page.items.first() {
        let t1 = Instant::now();
        let detail = character::get_character_detail(&data_dir, &db, &first.id).expect("detail");
        println!(
            "detail {} ({} skins): {:.2?}",
            detail.name,
            detail.skin_count,
            t1.elapsed()
        );
    }
}
