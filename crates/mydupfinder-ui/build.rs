//! Embeds the application icon and version information into the Windows
//! executable. A no-op on every other host OS.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/icons/icon.ico");

    #[cfg(windows)]
    embed_windows_resources();
}

#[cfg(windows)]
fn embed_windows_resources() {
    // Only when the *target* is Windows (the build script runs on the host).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/icons/icon.ico")
        .set("ProductName", "myDupFinder UI")
        .set(
            "FileDescription",
            "Native viewer for myDupFinder dupReport.csv files",
        )
        .set("CompanyName", "Roger Briggen")
        .set("LegalCopyright", "Copyright (c) Roger Briggen")
        .set("OriginalFilename", "mydupfinder-ui.exe");
    // FileVersion / ProductVersion come from the package version automatically.
    if let Err(err) = res.compile() {
        println!("cargo:warning=failed to embed Windows resources: {err}");
        std::process::exit(1);
    }
}
