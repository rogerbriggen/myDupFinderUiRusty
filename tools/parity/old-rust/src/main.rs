//! Parity harness, OLD side: the unchanged Tauri backend modules of the old
//! myDupFinderUi repo (parser, query, identical detector), included via #[path].
#![allow(dead_code, unused_imports)]

// The old modules (error, model, csv_parser, query, identical), see build.rs.
include!(concat!(env!("OUT_DIR"), "/old_mods.rs"));

use serde_json::{Value, json};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (csv, out) = (Path::new(&args[1]), Path::new(&args[2]));
    let rows = csv_parser::parse_dup_report_with_progress(csv, |_| {}).expect("old parser failed");
    let has_second = rows.iter().any(|r| r.source == model::Source::Second);
    let job = csv_parser::job_name_from_path(csv);

    if args.len() < 4 {
        // Phase 1: rows (as the old frontend received them) + identical pairs.
        let rows_json: Vec<Value> = rows
            .iter()
            .map(|r| json!({"filenameAndPath": r.filename_and_path, "fileSize": r.file_size,
                "fileSha512Hash": r.file_sha512_hash, "category": r.category,
                "source": r.source, "groupId": r.group_id}))
            .collect();
        std::fs::write(out.join("old_rows.json"),
            serde_json::to_string(&json!({"job": job, "hasSecond": has_second, "rows": rows_json})).unwrap()).unwrap();
        let pairs = identical::find_identical_folders(&rows);
        let pairs: Vec<Value> = pairs.iter()
            .map(|p| json!([p.folder_a, p.folder_b, p.file_count, p.total_size])).collect();
        std::fs::write(out.join("old_pairs.json"), serde_json::to_string(&pairs).unwrap()).unwrap();
        return;
    }

    // Phase 2: run every query through the old Rust `list_rows` (Tauri path).
    let queries: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(&args[3]).unwrap()).unwrap();
    let mut results = Vec::with_capacity(queries.len());
    for q in &queries {
        let parse_cats = |v: &Value| v.as_array().map(|a| a.iter()
            .map(|c| model::Category::parse(c.as_str().unwrap()).unwrap()).collect());
        let parse_srcs = |v: &Value| v.as_array().map(|a| a.iter()
            .map(|c| model::Source::parse(c.as_str().unwrap()).unwrap()).collect());
        let query = model::RowQuery {
            folder: q["folder"].as_str().map(String::from),
            include_descendants: q["includeDescendants"].as_bool().unwrap_or(false),
            text: q["text"].as_str().map(String::from),
            hash: q["hash"].as_str().map(String::from),
            categories: parse_cats(&q["categories"]),
            sources: parse_srcs(&q["sources"]),
            offset: 0,
            limit: usize::MAX,
        };
        let page = query::list_rows(&rows, &query);
        // Map the returned (cloned, file-ordered) rows back to row indices.
        let mut idx = Vec::with_capacity(page.rows.len());
        let mut i = 0;
        for r in &page.rows {
            while !(rows[i].filename_and_path == r.filename_and_path && rows[i].source == r.source
                && rows[i].group_id == r.group_id && rows[i].file_size == r.file_size) {
                i += 1;
            }
            idx.push(i);
            i += 1;
        }
        assert_eq!(page.total, idx.len());
        results.push(idx);
    }
    std::fs::write(out.join("old_rust_results.json"), serde_json::to_string(&results).unwrap()).unwrap();
}
