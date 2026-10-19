fn main() {
    println!("cargo:rerun-if-changed=../../assets/icon/JoystickGobloon.ico");
    println!("cargo:rerun-if-env-changed=JOYSTICK_GOBLOON_VERSION");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/icon/JoystickGobloon.ico");
        // Version info shown by Explorer and Task Manager: the product name, never the
        // crate name.
        let version = std::env::var("JOYSTICK_GOBLOON_VERSION")
            .unwrap_or_else(|_| std::env::var("CARGO_PKG_VERSION").unwrap_or_default());
        res.set("ProductName", "Joystick Gobloon")
            .set("FileDescription", "Joystick Gobloon")
            .set("InternalName", "JoystickGobloon")
            .set("OriginalFilename", "JoystickGobloon.exe")
            .set("CompanyName", "Denis Ibragimov")
            .set(
                "LegalCopyright",
                "Copyright (c) 2026 Denis Ibragimov. MIT License.",
            )
            .set("ProductVersion", &version);
        res.compile().expect("failed to embed Windows resources");
    }
}
