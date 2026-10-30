// The same version as Joystick Gobloon's (`version.rs` there): the program compares them to
// offer an update of the installed service.
include!("../joystick-gobloon/version.rs");

fn main() {
    println!("cargo:rerun-if-env-changed=JOYSTICK_GOBLOON_VERSION");
    println!("cargo:rustc-env=JOYSTICK_GOBLOON_VERSION={}", version());
}
