use ling_chat_main::ai_service::game_system::script_engine::validate::{
    Diagnostic, ExtensionEventContext, Severity, check_choices, push_target,
};
use serde_json::Value as JsonValue;
use std::{collections::HashSet, path::Path};
pub fn config(data_dir: &Path, script_dir: &Path, config: &JsonValue, diags: &mut Vec<Diagnostic>) {
    if let Some(main_character) = config.get("main_character").and_then(|v| v.as_str()) {
        let main_character = main_character.trim();
        let safe_folder = !main_character.is_empty()
            && Path::new(main_character).components().count() == 1
            && Path::new(main_character)
                .file_name()
                .and_then(|v| v.to_str())
                == Some(main_character);
        let settings = data_dir
            .join("game_data")
            .join("characters")
            .join(main_character)
            .join("settings.yml");
        if !safe_folder
            || (!settings.is_file()
                && !script_dir
                    .join("characters")
                    .join(main_character)
                    .join("settings.yml")
                    .is_file())
        {
            diags.push(Diagnostic::script(
                Severity::Error,
                "config.main_character_missing",
                format!(
                    "main_character「{}」必须匹配剧本或全局 characters/ 下含 settings.yml 的目录名",
                    main_character
                ),
            ));
        }
    }
    if let Some(value) = config.get("content_warning") {
        if value.as_str().is_none_or(|warning| warning != "horror") {
            diags.push(Diagnostic::script(
                Severity::Warn,
                "config.bad_content_warning",
                "content_warning 当前只支持字符串 horror".to_string(),
            ));
        }
    }
    if config
        .get("editor_locked")
        .is_some_and(|value| !value.is_boolean())
    {
        diags.push(Diagnostic::script(
            Severity::Warn,
            "config.bad_editor_locked",
            "editor_locked 必须是布尔值".to_string(),
        ));
    }
}
pub fn event(event_type: &str, ctx: &mut ExtensionEventContext<'_, '_>) {
    let ExtensionEventContext {
        config,
        obj,
        cid,
        i,
        chapter_set,
        diags,
        vars_written,
        vars_read,
        edges,
    } = ctx;
    let i = *i;
    let horror_system_effects_allowed = config
        .get("content_warning")
        .and_then(|v| v.as_str())
        .is_some_and(|warning| warning == "horror")
        && config
            .get("script_settings")
            .and_then(|v| v.get("allow_system_effects"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
    let declared_character_files: HashSet<String> = config
        .get("script_settings")
        .and_then(|v| v.get("character_files"))
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();

    match event_type {
        "force_choice" => {
            check_choices(obj, cid, i, diags, vars_written, vars_read);
            let forced = obj.get("forced").and_then(|v| v.as_str()).unwrap_or("");
            let matches = obj
                .get("options")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .filter_map(|option| option.get("text").and_then(|v| v.as_str()))
                .filter(|text| *text == forced)
                .count();
            if forced.is_empty() || matches != 1 {
                diags.push(
                    Diagnostic::event(
                        Severity::Error,
                        "force_choice.bad_forced",
                        cid,
                        i,
                        "forced 必须精确匹配且只匹配一个选项文本".to_string(),
                    )
                    .with_field("forced"),
                );
            }
        },
        "random_var" => {
            if let Some(variable) = obj.get("variable").and_then(|v| v.as_str()) {
                if !variable.trim().is_empty() {
                    vars_written.insert(variable.trim().to_string());
                }
            }
            if let Some(chance) = obj.get("chance").and_then(|v| v.as_f64()) {
                if !(0.0..=1.0).contains(&chance) {
                    diags.push(
                        Diagnostic::event(
                            Severity::Warn,
                            "random_var.clamped_chance",
                            cid,
                            i,
                            "chance 超出 0–1，运行时会截断".to_string(),
                        )
                        .with_field("chance"),
                    );
                }
            }
        },
        "character_file" => {
            let action = obj
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("exists");
            if !["ensure", "exists", "delete", "open_folder"].contains(&action) {
                diags.push(Diagnostic::event(
                    Severity::Error,
                    "character_file.bad_action",
                    cid,
                    i,
                    format!("未知角色文件操作「{}」", action),
                ));
            }
            if action != "open_folder" {
                let file = obj.get("file").and_then(|v| v.as_str()).unwrap_or("");
                if !declared_character_files.contains(file) {
                    diags.push(
                        Diagnostic::event(
                            Severity::Error,
                            "character_file.not_declared",
                            cid,
                            i,
                            format!("角色文件「{}」未在 story_config 白名单中声明", file),
                        )
                        .with_field("file"),
                    );
                }
            }
            if let Some(variable) = obj
                .get("resultVar")
                .or_else(|| obj.get("result_var"))
                .and_then(|v| v.as_str())
            {
                if !variable.trim().is_empty() {
                    vars_written.insert(variable.trim().to_string());
                }
            }
        },
        "watch_file" => {
            let action = obj
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("start");
            if action == "start" {
                let file = obj.get("file").and_then(|v| v.as_str()).unwrap_or("");
                if !declared_character_files.contains(file) {
                    diags.push(
                        Diagnostic::event(
                            Severity::Error,
                            "watch_file.not_declared",
                            cid,
                            i,
                            format!("监视文件「{}」未在 story_config 白名单中声明", file),
                        )
                        .with_field("file"),
                    );
                }
                let target = obj.get("on_missing").and_then(|v| v.as_str()).unwrap_or("");
                push_target(
                    target,
                    "文件消失跳转",
                    "watch_file",
                    "watch_file",
                    cid,
                    i,
                    chapter_set,
                    edges,
                    diags,
                );
            } else if action != "stop" {
                diags.push(Diagnostic::event(
                    Severity::Error,
                    "watch_file.bad_action",
                    cid,
                    i,
                    format!("watch_file action 只支持 start / stop，收到「{}」", action),
                ));
            }
        },
        "poem_game" => {
            vars_written.extend(
                [
                    "poem_glitch",
                    "poem_warm_score",
                    "poem_script_score",
                    "poem_void_score",
                ]
                .map(str::to_owned),
            );
            if let Some(variable) = obj.get("resultVar").and_then(|v| v.as_str()) {
                if !variable.trim().is_empty() {
                    vars_written.insert(variable.trim().to_string());
                }
            }
        },
        "console_window" => {
            if !horror_system_effects_allowed {
                diags.push(Diagnostic::event(
                    Severity::Error,
                    "console_window.not_authorized",
                    cid,
                    i,
                    "console_window 需要 content_warning: horror 且 allow_system_effects: true"
                        .to_string(),
                ));
            }
        },
        "main_menu_effect" => {
            let theme = obj
                .get("theme")
                .and_then(|v| v.as_str())
                .unwrap_or("normal");
            if !["normal", "blood", "ghost"].contains(&theme) {
                diags.push(
                    Diagnostic::event(
                        Severity::Error,
                        "main_menu_effect.bad_theme",
                        cid,
                        i,
                        format!("主菜单主题「{}」不受支持", theme),
                    )
                    .with_field("theme"),
                );
            }
        },
        _ => {},
    }
}
