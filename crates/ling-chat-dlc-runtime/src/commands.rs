//! A single list controls both command discovery and Tauri dispatch.
macro_rules! commands {
    ($($command:path),* $(,)?) => {
        pub fn handler(name: &str) -> Option<fn(tauri::ipc::Invoke<tauri::Wry>) -> bool> {
            let names = [$(stringify!($command).rsplit("::").next().unwrap().trim()),*];
            names.contains(&name).then_some(tauri::generate_handler![$($command),*])
        }
    };
}
commands![
    crate::games::catalog::game_catalog_cached,
    crate::games::catalog::game_catalog_sync,
    crate::games::catalog::game_catalog_install,
    crate::games::runtime::game_open,
    crate::games::runtime::game_close,
    crate::games::runtime::game_resize,
    crate::games::runtime::game_seed_save,
    crate::story::commands::get_script_menu_effect,
    crate::story::commands::check_script_ghost_lock,
    crate::story::commands::script_submit_poem,
    crate::story::commands::show_script_glitch_window,
    crate::story::commands::spawn_script_console_window,
    crate::story::commands::reset_script_state,
    crate::story::commands::get_script_cursor_position,
    crate::story::commands::warp_cursor,
    crate::story::commands::cancel_script_cursor_warp,
];

#[cfg(test)]
mod tests {
    #[test]
    fn only_registered_extension_commands_are_claimed() {
        for name in [
            "game_open",
            "game_close",
            "script_submit_poem",
            "warp_cursor",
            "get_script_menu_effect",
        ] {
            assert!(super::handler(name).is_some(), "{name}");
        }
        for name in ["start_script", "exit_app", "plugin_delete", "game_unknown"] {
            assert!(super::handler(name).is_none(), "{name}");
        }
    }
}
