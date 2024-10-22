fn main() {
    if cfg!(target_os = "windows") {
        let mut res = winres::WindowsResource::new();

        res.set_manifest(include_str!("./Manifest.xml"));
        res.set_icon("src/resources/icon.ico");
        res.compile().unwrap();
    }
}
