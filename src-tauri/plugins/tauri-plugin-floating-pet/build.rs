const COMMANDS: &[&str] = &[
    "is_supported",
    "check_permission",
    "request_permission",
    "show",
    "hide",
    "move_pet",
    "set_touchable",
    "set_expanded",
    "set_size",
    "is_visible",
    "status",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
