const COMMANDS: &[&str] = &[
    "check_overlay_permission",
    "request_overlay_permission",
    "show_floating_pet",
    "hide_floating_pet",
    "stop_floating_pet_service",
    "stop_floating_pet_service_with_confirmation",
    "update_pet_state",
    "start_permission_explanation",
    "mark_permission_explanation_shown",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
