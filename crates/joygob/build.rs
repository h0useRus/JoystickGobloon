fn main() {
    println!("cargo:rerun-if-changed=../../assets/icon/joygob.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/icon/joygob.ico");
        res.compile().expect("failed to embed Windows resources");
    }
}
