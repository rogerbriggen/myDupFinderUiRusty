//! Parity harness, NEW side: dupreport-core of the GPUI rewrite.
use dupreport_core::{Category, IdenticalStatus, Report, RowQuery, Source, compute_statuses, filter, find_identical_folders};
use serde_json::{Map, Value, json};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (csv, out) = (Path::new(&args[1]), Path::new(&args[2]));
    let report = Report::load(csv, |_| {}).expect("new parser failed");

    let rows: Vec<Value> = report.rows.iter().map(|r| json!({"filenameAndPath": r.path, "fileSize": r.size,
        "fileSha512Hash": r.hash, "category": r.category.as_str(), "source": r.source.as_str(), "groupId": r.group_id})).collect();

    let mut tree = Map::new();
    for n in &report.tree.nodes {
        tree.insert(n.path.clone(), json!([n.direct_file_count, n.total_file_count, n.total_size,
            Category::ALL.iter().map(|c| n.category_counts[*c as usize]).collect::<Vec<_>>(),
            n.children.iter().map(|&c| report.tree.node(c).name.clone()).collect::<Vec<_>>()]));
    }

    let pairs = find_identical_folders(&report.rows);
    let statuses = compute_statuses(&report.tree, &pairs);
    let mut st = Map::new();
    for (n, s) in report.tree.nodes.iter().zip(&statuses) {
        if let Some(s) = s {
            st.insert(n.path.clone(), json!(match s { IdenticalStatus::Green => "green", IdenticalStatus::Yellow => "yellow", IdenticalStatus::Red => "red" }));
        }
    }

    let queries: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(&args[3]).unwrap()).unwrap();
    let results: Vec<Vec<u32>> = queries.iter().map(|q| {
        let cats: Vec<Category> = q["cats"].as_array().unwrap().iter().map(|c| Category::parse(c.as_str().unwrap()).unwrap()).collect();
        let srcs: Vec<Source> = q["srcs"].as_array().unwrap().iter().map(|c| Source::parse(c.as_str().unwrap()).unwrap()).collect();
        // Exactly what ReportStore::query() builds: trimmed, empty -> None,
        // full category / source set -> None.
        let opt = |v: &Value| v.as_str().map(str::trim).filter(|s| !s.is_empty()).map(String::from);
        filter(&report, &RowQuery {
            folder: Some(q["folder"].as_str().unwrap().to_string()),
            include_descendants: q["desc"].as_bool().unwrap(),
            text: opt(&q["text"]), hash: opt(&q["hash"]),
            categories: (cats.len() < Category::ALL.len()).then_some(cats),
            sources: (srcs.len() < Source::ALL.len()).then_some(srcs),
        }).into_iter().map(|i| i as u32).collect()
    }).collect();

    std::fs::write(out.join("new.json"), serde_json::to_string(&json!({
        "job": report.job_name, "hasSecond": report.has_second_source, "rows": rows, "tree": tree,
        "pairs": pairs.iter().map(|p| json!([p.folder_a, p.folder_b, p.file_count, p.total_size])).collect::<Vec<_>>(),
        "statuses": st, "results": results,
    })).unwrap()).unwrap();
}
