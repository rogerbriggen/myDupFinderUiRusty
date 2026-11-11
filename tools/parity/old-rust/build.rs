//! Points the harness at the old repo's unchanged backend sources.
//! `OLD_REPO` = checkout of github.com/rogerbriggen/myDupFinderUi
//! (default: a sibling of this repo, `../myDupFinderUi`).

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=OLD_REPO");
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let old_repo = std::env::var_os("OLD_REPO")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../../../../myDupFinderUi"));
    let src = old_repo.join("src-tauri/src");
    let src = src
        .canonicalize()
        .unwrap_or_else(|e| panic!("old backend sources not found at {}: {e} (set OLD_REPO)", src.display()));
    let mut mods = String::new();
    for m in ["error", "model", "csv_parser", "query", "identical"] {
        let file = src.join(format!("{m}.rs"));
        println!("cargo:rerun-if-changed={}", file.display());
        // `{:?}` gives an escaped string literal, safe for Windows paths.
        mods.push_str(&format!("#[path = {:?}]\nmod {m};\n", file.to_str().unwrap()));
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("old_mods.rs");
    std::fs::write(out, mods).unwrap();
}
