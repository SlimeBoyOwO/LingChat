use ling_chat_main::ai_service::game_system::script_engine::schema::{
    EventSpec, FieldKind, FieldSpec,
};
pub fn events() -> Vec<EventSpec> {
    vec![
        // ---------- DLC / 恐怖剧本扩展 ----------
        EventSpec {
            type_key: "force_choice",
            label: "强制选项",
            category: "交互",
            color: "#ef4444",
            fields: vec![
                FieldSpec::new("options", "选项列表", FieldKind::ChoiceOptions)
                    .required()
                    .hint("与普通 choices 相同，可给选项配置 condition / lock_hint"),
                FieldSpec::new("forced", "强制选择文本", FieldKind::Text)
                    .required()
                    .hint("必须与一个未锁定选项的 text 完全一致"),
            ],
        },
        EventSpec {
            type_key: "poem_game",
            label: "写诗小游戏",
            category: "交互",
            color: "#f472b6",
            fields: vec![
                FieldSpec::new("backgroundPath", "写诗背景", FieldKind::Asset)
                    .required()
                    .asset("background"),
                FieldSpec::new("musicPath", "正常音乐", FieldKind::Asset)
                    .required()
                    .asset("music"),
                FieldSpec::new("glitchMusicPath", "崩坏音乐", FieldKind::Asset)
                    .required()
                    .asset("music"),
                FieldSpec::new("warmStickerPath", "温暖角色贴纸", FieldKind::Asset)
                    .required()
                    .asset("pic"),
                FieldSpec::new("scriptStickerPath", "剧本角色贴纸", FieldKind::Asset)
                    .required()
                    .asset("pic"),
                FieldSpec::new("voidStickerPath", "空白角色贴纸", FieldKind::Asset)
                    .required()
                    .asset("pic"),
                FieldSpec::new("wordListPath", "词库文件", FieldKind::Text)
                    .required()
                    .placeholder("poem_words.yaml")
                    .hint("只能填写剧本根目录下的文件名"),
                FieldSpec::new("resultVar", "结果变量", FieldKind::Text).placeholder("poem_tone"),
                FieldSpec::new("rounds", "轮数", FieldKind::Number)
                    .placeholder("20")
                    .hint("引擎限制 1–20"),
                FieldSpec::new("glitch", "强制崩坏词", FieldKind::Bool),
                FieldSpec::new("mode", "写诗模式", FieldKind::Select)
                    .options(["normal", "act2", "act2_final"])
                    .default_desc("normal"),
            ],
        },
        EventSpec {
            type_key: "wait",
            label: "等待",
            category: "流程",
            color: "#64748b",
            fields: vec![
                FieldSpec::new("seconds", "等待秒数", FieldKind::Number)
                    .placeholder("1.0")
                    .hint("用于无需玩家点击的演出停顿"),
            ],
        },
        EventSpec {
            type_key: "random_var",
            label: "随机变量",
            category: "流程",
            color: "#a855f7",
            fields: vec![
                FieldSpec::new("variable", "变量名", FieldKind::Text).required(),
                FieldSpec::new("chance", "为 true 的概率", FieldKind::Number)
                    .placeholder("0.5")
                    .hint("范围 0–1，超出时会截断"),
            ],
        },
        EventSpec {
            type_key: "character_file",
            label: "角色文件",
            category: "流程",
            color: "#dc2626",
            fields: vec![
                FieldSpec::new("action", "操作", FieldKind::Select)
                    .required()
                    .options(["ensure", "exists", "delete", "open_folder"]),
                FieldSpec::new("file", "标记文件", FieldKind::Text)
                    .hint("必须在 story_config.script_settings.character_files 白名单中"),
                FieldSpec::new("resultVar", "检测结果变量", FieldKind::Text)
                    .hint("exists 操作把布尔结果写入该变量"),
                FieldSpec::new("result_var", "旧结果变量字段", FieldKind::Deprecated)
                    .disabled("兼容旧剧本；新剧本请使用 resultVar"),
            ],
        },
        EventSpec {
            type_key: "watch_file",
            label: "监视角色文件",
            category: "流程",
            color: "#b91c1c",
            fields: vec![
                FieldSpec::new("action", "操作", FieldKind::Select)
                    .options(["start", "stop"])
                    .default_desc("start"),
                FieldSpec::new("file", "监视文件", FieldKind::Text)
                    .hint("start 时必填，且必须在角色文件白名单中"),
                FieldSpec::new("on_missing", "文件消失后跳转", FieldKind::Chapter)
                    .hint("start 时必填"),
            ],
        },
        EventSpec {
            type_key: "voice_shift",
            label: "语音变调",
            category: "声音",
            color: "#7c3aed",
            fields: vec![
                FieldSpec::new("rate", "播放倍率", FieldKind::Number)
                    .placeholder("1.0")
                    .hint("引擎限制 0.5–1.5；会同时改变语速与音调"),
                FieldSpec::new("pitch", "纯音高偏移（半音）", FieldKind::Number)
                    .placeholder("0")
                    .hint("引擎限制 -12–12；通过 Web Audio detune 实现"),
            ],
        },
        EventSpec {
            type_key: "horror_log",
            label: "恐怖日志刷屏",
            category: "演出",
            color: "#991b1b",
            fields: vec![
                FieldSpec::new("text", "日志文本", FieldKind::Textarea).required(),
                FieldSpec::new("lines", "重复行数", FieldKind::Number)
                    .placeholder("1")
                    .hint("引擎会限制最大行数"),
            ],
        },
        EventSpec {
            type_key: "console_window",
            label: "原生系统窗口",
            category: "演出",
            color: "#450a0a",
            fields: vec![
                FieldSpec::new("title", "窗口标题", FieldKind::Text),
                FieldSpec::new("text", "窗口正文", FieldKind::Textarea).required(),
                FieldSpec::new("style", "窗口样式", FieldKind::Select)
                    .options(["console", "blood_cmd", "error", "warning", "notepad"])
                    .default_desc("console"),
                FieldSpec::new("count", "窗口数量", FieldKind::Number)
                    .placeholder("1")
                    .hint("引擎限制 1–4"),
                FieldSpec::new("interval", "窗口间隔（秒）", FieldKind::Number).placeholder("0.25"),
                FieldSpec::new("lifetime", "最长存活（秒）", FieldKind::Number)
                    .placeholder("4")
                    .hint("引擎限制 1–12 秒；玩家可提前关闭"),
            ],
        },
        EventSpec {
            type_key: "jumpscare",
            label: "突脸惊吓",
            category: "演出",
            color: "#7f1d1d",
            fields: vec![
                FieldSpec::new("imagePath", "突脸图片", FieldKind::Asset)
                    .required()
                    .asset("pic"),
                FieldSpec::new("soundPath", "惊吓音效", FieldKind::Asset).asset("sound"),
            ],
        },
        EventSpec {
            type_key: "glitch_window",
            label: "应用内故障窗口",
            category: "演出",
            color: "#6b21a8",
            fields: vec![
                FieldSpec::new("title", "窗口标题", FieldKind::Text),
                FieldSpec::new("text", "窗口正文", FieldKind::Textarea).required(),
                FieldSpec::new("style", "样式", FieldKind::Select)
                    .required()
                    .options(["terminal", "error"]),
                FieldSpec::new("count", "窗口数量", FieldKind::Number).placeholder("1"),
                FieldSpec::new("interval", "窗口间隔（秒）", FieldKind::Number).placeholder("0.25"),
                FieldSpec::new("lifetime", "存活时间（秒）", FieldKind::Number).placeholder("4"),
            ],
        },
        EventSpec {
            type_key: "window_title",
            label: "窗口标题故障",
            category: "演出",
            color: "#4c1d95",
            fields: vec![
                FieldSpec::new("title", "窗口标题", FieldKind::Text)
                    .hint("留空可把标题切回应用默认值"),
            ],
        },
        EventSpec {
            type_key: "main_menu_effect",
            label: "主菜单主题",
            category: "演出",
            color: "#581c87",
            fields: vec![
                FieldSpec::new("theme", "主题", FieldKind::Select)
                    .required()
                    .options(["normal", "blood", "ghost"]),
                FieldSpec::new("message", "菜单短句", FieldKind::Textarea)
                    .hint("最多 160 字；主题状态按剧本归属持久化"),
            ],
        },
    ]
}

/// Extend the editor contract without putting content-specific fields in LC.
pub fn configure(
    schema: &mut ling_chat_main::ai_service::game_system::script_engine::schema::ScriptSchema,
) {
    schema.events.extend(events());
    schema.story_config_fields.extend(vec![
        FieldSpec::new("main_character", "剧本主角目录", FieldKind::Text)
            .hint("正式进入时切换并锁定到全局 characters/ 下的目录名；结束后恢复原主角"),
        FieldSpec::new("content_warning", "内容警告类型", FieldKind::Select)
            .options(["horror"])
            .hint("horror 会在进入前要求玩家确认，并允许受限系统演出授权"),
        FieldSpec::new("editor_locked", "禁止编辑", FieldKind::Bool)
            .default_desc("false")
            .hint("发行版 DLC 可锁定编辑器修改，但仍允许校验和正式游玩"),
    ]);
    for event in &mut schema.events {
        match event.type_key {
            "modify_character" => event.fields.extend(vec![
                FieldSpec::new("flash", "立绘故障闪现", FieldKind::Bool)
                    .default_desc("false")
                    .hint("硬切闪现指定情绪立绘，供短促惊吓演出"),
                FieldSpec::new("noise", "立绘噪点区域", FieldKind::Select)
                    .options(["none", "eyes", "mouth", "eyes_mouth"])
                    .default_desc("none")
                    .hint("黑噪点侵蚀眼睛/嘴部；none 清除当前侵蚀"),
                FieldSpec::new("noise_fade_in", "噪点淡入（秒）", FieldKind::Number)
                    .placeholder("0")
                    .hint("仅 noise 非 none 时生效；0 表示立即出现"),
            ]),
            "background_effect" => {
                event.fields.extend(vec![
                    FieldSpec::new("text", "特效文本", FieldKind::Textarea)
                        .hint("UiCorrupt 等文字侵蚀特效的显示内容"),
                    FieldSpec::new("echo", "回声文本", FieldKind::Text)
                        .hint("可选的故障回声/残影文本"),
                ]);
                if let Some(field) = event.fields.iter_mut().find(|f| f.key == "effect") {
                    field
                        .options
                        .extend(effect_names().into_iter().map(str::to_owned));
                }
            },
            _ => {},
        }
    }
}
pub fn effect_names() -> Vec<&'static str> {
    vec![
        "Glitch",
        "Shake",
        "Flash",
        "Blackout",
        "Tear",
        "Static",
        "Invert",
        "BloodDrip",
        "Veins",
        "BSOD",
        "UiCorrupt",
        "BloodUI",
    ]
}
