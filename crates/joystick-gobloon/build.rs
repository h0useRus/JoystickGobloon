fn main() {
    println!("cargo:rerun-if-changed=../../assets/icon/JoystickGobloon.ico");
    println!("cargo:rerun-if-env-changed=JOYSTICK_GOBLOON_VERSION");
    let version = version();
    // Read by `VERSION` in main.rs.
    println!("cargo:rustc-env=JOYSTICK_GOBLOON_VERSION={version}");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/icon/JoystickGobloon.ico");
        // Version info shown by Explorer and Task Manager: the product name, never the
        // crate name.
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

// `version()`: shared with SystemGobloon, whose version must match.
include!("version.rs");
