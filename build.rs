fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_manifest_file("assets/app.manifest");
        res.set_icon("assets/app.ico");
        res.compile().expect("failed to compile Windows resources");
    }
}
