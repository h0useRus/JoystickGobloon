fn main() {
    println!("cargo:rerun-if-changed=../../assets/icon/JoystickGobloon.ico");
    println!("cargo:rerun-if-env-changed=JOYSTICK_GOBLOON_VERSION");
    let version = version();
    // Read by `VERSION` in main.rs.
    println!("cargo:rustc-env=JOYSTICK_GOBLOON_VERSION={version}");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/icon/JoystickGobloon.ico");
        res.set("ProductName", "Map Gobloon")
            .set("FileDescription", "Map Gobloon")
            .set("InternalName", "MapGobloon")
            .set("OriginalFilename", "MapGobloon.exe")
            .set("CompanyName", "Denis Ibragimov")
            .set(
                "LegalCopyright",
                "Copyright (c) 2026 Denis Ibragimov. MIT License.",
            )
            .set("ProductVersion", &version);
        res.compile().expect("failed to embed Windows resources");
    }
}

// The same version as Joystick Gobloon's: they ship together.
include!("../joystick-gobloon/version.rs");
