fn main() {
    // Only embed the icon when building for Windows
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("icons/icon.ico");
        res.compile().expect("Failed to compile Windows resources. Ensure icons/icon.ico exists and is a valid ICO file.");
    }
}
