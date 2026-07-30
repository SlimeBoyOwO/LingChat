// 本文件由 scripts/generate-zh-hk.mjs 自动生成（源：zh-CN/settings.ts），请勿手改
export default {
  "shared": {
    "prevPage": "上一頁",
    "nextPage": "下一頁",
    "loading": "加載中...",
    "pageOf": "第 {current} / {total} 頁",
    "pageOfTotal": "第 {current} 頁 / 共 {total} 頁"
  },
  "history": {
    "title": "歷史對話",
    "empty": "暫無歷史記錄，去和ta聊聊天叭(*^▽^*)",
    "backtrackTip": "回溯到此消息之前（將清除此消息及之後所有對話）",
    "backtrack": "回溯",
    "thinking": "思考過程（{count} 字）",
    "playVoice": "播放語音",
    "you": "你",
    "mysteryVoice": "謎之音",
    "backtrackConfirm": "確定要回溯到此對話嗎？此操作將清除該消息及之後的所有對話，且不可撤銷。",
    "backtrackConfirmTitle": "回溯確認",
    "backtrackFailed": "回溯失敗：{error}",
    "emotionTitle": "情感: {tag}"
  },
  "achievement": {
    "title": "成就列表（實驗）",
    "rare": "稀有",
    "normal": "普通"
  },
  "adventure": {
    "standalone": {
      "title": "獨立劇本",
      "empty": "暫無獨立劇本",
      "emptyDesc": "獨立劇本是無需選擇角色即可遊玩的劇本",
      "badge": "獨立劇本",
      "noDesc": "暫無描述",
      "chapterSelect": "章節選擇（待做）: {chapter}",
      "play": "開始遊玩"
    },
    "bond": {
      "title": "羈絆冒險（測試中）",
      "noCharacter": "請先在角色頁面選擇一個角色",
      "noCharacterDesc": "選擇角色後即可查看其羈絆冒險",
      "goCharacter": "前往角色頁面",
      "avatarAlt": "角色頭像",
      "noSubtitle": "暫無副標題",
      "switchCharacter": "切換角色"
    },
    "workshop": {
      "title": "創意工坊",
      "enter": "進入創意工坊"
    },
    "createScript": {
      "title": "創建自己的劇本",
      "guide": "訪問指南網站"
    }
  },
  "character": {
    "list": {
      "title": "角色列表（切換角色會開始全新對話）",
      "noDesc": "暫無角色描述"
    },
    "openFolder": {
      "title": "打開人物文件夾",
      "button": "打開人物文件夾"
    },
    "import": {
      "title": "從壓縮包導入角色 (.zip / .7z)",
      "conflictPolicy": "同名衝突策略",
      "policyRename": "自動重命名（默認）",
      "policySkip": "跳過已存在的",
      "policyOverwrite": "覆蓋已存在的",
      "button": "選擇壓縮包導入"
    },
    "refresh": {
      "title": "刷新人物列表",
      "button": "點我刷新"
    },
    "workshop": {
      "title": "創意工坊",
      "enter": "進入創意工坊"
    }
  },
  "log": {
    "title": "日誌",
    "resume": "繼續",
    "pause": "暫停",
    "clear": "清空",
    "empty": "暫無日誌",
    "paused": "已暫停 — {count} 條新日誌"
  },
  "save": {
    "create": {
      "title": "創建新存檔（會記錄當前對話）",
      "placeholder": "輸入存檔名稱",
      "creating": "創建中...",
      "button": "創建"
    },
    "list": {
      "title": "存檔列表",
      "loadFailed": "加載失敗: {error}",
      "empty": "暫無存檔記錄",
      "noScreenshot": "暫無截圖",
      "editTitleTip": "雙擊以修改存檔標題",
      "untitled": "未命名存檔",
      "noMessage": "暫無對話台詞記錄"
    },
    "action": {
      "reading": "讀取中...",
      "load": "讀取存檔",
      "saving": "保存中...",
      "overwrite": "覆蓋存檔",
      "deleting": "刪除中...",
      "delete": "刪除存檔"
    },
    "msg": {
      "warnTitle": "提示",
      "nameRequired": "存檔名稱不能為空",
      "renameSuccessTitle": "修改成功",
      "renameSuccessMsg": "存檔名稱已修改",
      "renameFailTitle": "修改失敗",
      "unknownError": "未知錯誤",
      "nameEmpty": "請輸入存檔名稱",
      "createSuccessTitle": "創建成功",
      "createSuccessMsg": "存檔已創建",
      "createFailTitle": "創建失敗",
      "loadConfirm": "加載存檔會導致丟失當前對話進度，確定要加載嗎？",
      "loadSuccessTitle": "加載成功",
      "loadSuccessMsg": "存檔已加載",
      "loadFailTitle": "加載失敗",
      "overwriteConfirm": "覆蓋存檔會導致丟失之前的存檔進度，確定要覆蓋嗎？",
      "overwriteSuccessTitle": "保存成功",
      "overwriteSuccessMsg": "存檔已覆蓋",
      "overwriteFailTitle": "保存失敗",
      "deleteConfirm": "確定要刪除這個存檔嗎？此操作不可撤銷。",
      "deleteSuccessTitle": "刪除成功",
      "deleteSuccessMsg": "存檔已刪除",
      "deleteFailTitle": "刪除失敗"
    }
  },
  "advanceOther": {
    "restartHint": "💡 這裏的設置重啓軟件生效哦！",
    "backToList": "返回設置列表",
    "subcategoryDesc": "修改 {name} 的相關配置",
    "ttsControl": {
      "title": "TTS 連接控制",
      "desc": "TTS 服務重啓後，可在這裏解除離線狀態並讓下一次語音立即重新連接。模型、語言或接口參數錯誤導致的 HTTP 400 仍需修正對應配置。",
      "reconnecting": "正在重新連接…",
      "forceReconnect": "強制重新連接 TTS"
    },
    "saveButton": "保存",
    "loadFailed": "加載失敗",
    "loadFailedDesc": "無法加載配置或配置為空。",
    "msg": {
      "error": "錯誤: {error}",
      "ttsReactivating": "正在重新啓用 TTS 服務…",
      "ttsReactivated": "TTS 已重新啓用，下一次語音會立即重試連接。",
      "ttsReconnectFailed": "重新連接失敗：{error}",
      "loadConfigFailed": "加載配置失敗: {error}"
    },
    "categories": {
      "LLM 配置": "LLM 配置",
      "翻译配置": "翻譯配置",
      "功能设置": "功能設置",
      "TTS 配置": "TTS 配置",
      "创意工坊": "創意工坊",
      "日志配置": "日誌配置",
      "主动对话配置": "主動對話配置"
    },
    "subcategories": {
      "高级选项": "高級選項",
      "功能选项": "功能選項",
      "对话增强": "對話增強",
      "记忆系统": "記憶系統",
      "适配器 URL": "適配器 URL",
      "音频参数": "音頻參數",
      "GitHub Token": "GitHub Token",
      "基础设置": "基礎設置",
      "基础开关": "基礎開關",
      "视觉感知设置": "視覺感知設置",
      "感知与话题配置": "感知與話題配置"
    },
    "subcategoryDescs": {
      "高级选项": "調優 AI 對話行為的高級參數",
      "功能选项": "翻譯功能的開關與行為控制",
      "对话增强": "這裏可以設置是否啓用時間感知和情緒分類器功能",
      "记忆系统": "在這裏設定你想要的永久記憶效果",
      "适配器 URL": "各個 TTS 後端的 API 地址，對應原環境變量 SIMPLE_VITS_API_URL / STYLE_BERT_VITS2_URL 等",
      "音频参数": "TTS 音頻輸出格式與語言設置，對應原環境變量 TTS_AUDIO_FORMAT / VOICE_LANG",
      "GitHub Token": "配置 GitHub Personal Access Token 以獲取準確的 Discussion upvote 熱度排序（可選）",
      "基础设置": "程序運行時文件日誌的相關設置",
      "基础开关": "主動對話功能的核心開關與觸發頻率設置",
      "视觉感知设置": "主動對話時的桌面視覺感知開關與觸發權重，視覺模型在大模型管理中配置",
      "感知与话题配置": "日程、TODO與隨機對話的權重及開關配置"
    },
    "fields": {
      "llm": {
        "output_sec_lang": "LLM_OUTPUT_SEC_LANG — 是否允許輸出第二語言（關閉後僅輸出中文）",
        "consumers": "COMSUMERS — 併發消費者數量（增大可加速流式輸出，默認 3）",
        "timeout_secs": "LLM 請求空閒超時（秒）— 首次響應及流式相鄰事件最長等待時間（10–3600）",
        "no_emotion_limit_prompt": "NO_EMOTION_LIMIT_PROMPT — 解除 emotion 數量限制（可能增加 token 消耗）"
      },
      "translate": {
        "enable": "ENABLE_TRANSLATE — 啓用 AI 翻譯（將中文對話翻譯為第二語言）"
      },
      "features": {
        "enable_time_sense": "USE_TIME_SENSE — 啓用時間感知（根據上下文時間添加系統提醒）",
        "enable_emotion_classifier": "ENABLE_EMOTION_CLASSIFIER — 啓用情感分類器（ONNX 模型，用於自動標註對話 emotion）",
        "use_persistent_memory": "USE_PERSISTENT_MEMORY — 開啓後記憶會自動壓縮，減少 token 消耗",
        "memory_update_interval": "MEMORY_UPDATE_INTERVAL — 觸發記憶摘要的新消息數（默認 250）",
        "memory_recent_window": "MEMORY_RECENT_WINDOW — 摘要時保留的最近消息數（默認 30）"
      },
      "tts": {
        "simple_vits_api_url": "Simple-Vits-API 地址（VITS 適配器）",
        "bv2_api_url": "Simple-Vits-API 地址（Bert-Vits2 適配器）",
        "gsv_api_url": "GPT-SoVITS API 地址",
        "sbv2_api_url": "Style-Bert-Vits2 本地服務地址",
        "sbv2api_api_url": "SBV2 API 服務地址",
        "aivis_api_url": "AIVIS 雲 API 地址",
        "aivis_api_key": "AIVIS API 密鑰（原環境變量 AIVIS_API_KRY）",
        "indextts_api_url": "IndexTTS2 API 地址",
        "opentts_api_url": "OpenTTS API 地址（硅基流動）",
        "opentts_api_key": "OpenTTS API 密鑰",
        "opentts_model": "OpenTTS 模型名稱",
        "opentts_voice": "OpenTTS voice / 音色標識",
        "audio_format": "音頻文件格式（wav / mp3 / flac / ogg 等）"
      },
      "workshop": {
        "github_token": "填入你的 GitHub Token（無需任何權限，僅用於調用 GraphQL API）。留空使用 REST API，無法獲取獨立 upvote 數（會用 👍 表情數代替）。Token 創建地址：https://github.com/settings/tokens"
      },
      "log": {
        "enable": "LOG_ENABLE — 是否將運行日誌寫入文件（位於 data/log/app/ 目錄）",
        "retention_days": "LOG_RETENTION_DAYS — 日誌文件保留天數，超過的舊文件在啓動時自動清理",
        "llm_request_body": "LOG_LLM_REQUEST_BODY — 記錄每次 LLM 請求的完整請求體 JSON 到 data/log/llm/ 目錄（默認關閉）"
      },
      "ENABLE_PROACTIVE_SYSTEM": "ENABLE_PROACTIVE_SYSTEM — 是否啓用主動對話系統",
      "MAX_PROACTIVE_TIMES": "MAX_PROACTIVE_TIMES — 在用户響應之前，能主動對話的次數",
      "ENABLE_VISUAL_PRECEPTION": "ENABLE_VISUAL_PRECEPTION — 是否允許主動視覺感知桌面畫面（偷看屏幕）",
      "SCREEN_WEIGHT": "SCREEN_WEIGHT — 視覺模式觸發權重（越大越容易偷看屏幕聊天，默認 30）",
      "ENABLE_TOPIC_CREATER": "ENABLE_TOPIC_CREATER — 允許自主尋找並開啓新話題",
      "TOPIC_WEIGHT": "TOPIC_WEIGHT — 隨機話題觸發權重（默認 60）",
      "ENABLE_TODO_PRECEPTION": "ENABLE_TODO_PRECEPTION — 允許在閒暇時自動讀取未完成 TODO 並温和提醒",
      "TODO_WEIGHT": "TODO_WEIGHT — TODO 提醒觸發權重（默認 10）",
      "ENABLE_SCHEDULE_REMINDER": "ENABLE_SCHEDULE_REMINDER — 啓用強日程日程報時彈窗提醒",
      "ENABLE_IMPORTANT_DAY_REMINDER": "ENABLE_IMPORTANT_DAY_REMINDER — 啓用重要節日與特殊日子暖心提醒"
    }
  },
  "workshop": {
    "title": "創意工坊",
    "all": "全部",
    "hot": "熱度",
    "newest": "最新",
    "loadingList": "正在加載討論列表...",
    "retry": "重試",
    "empty": "暫無討論內容",
    "emptyCategory": "該分類下暫無內容",
    "upvoteHint1": "當前無法獲取 Discussion 熱度（upvote）數據，列表按 👍 表情數排序。 在",
    "upvoteHintLink": "高級設置 → 創意工坊",
    "upvoteHint2": "中填入 GitHub Token 即可獲取精確的 upvote 熱度。",
    "upvoteTitle": "upvote 熱度",
    "reactionTitle": "👍 表情數",
    "unknownAuthor": "未知",
    "refreshList": "刷新列表",
    "noDesc": "暫無描述",
    "loadFailed": "加載失敗",
    "time": {
      "justNow": "剛剛",
      "minutesAgo": "{n} 分鐘前",
      "hoursAgo": "{n} 小時前",
      "daysAgo": "{n} 天前",
      "monthsAgo": "{n} 個月前",
      "yearsAgo": "{n} 年前"
    }
  },
  "adventurePanel": {
    "header": {
      "title": "冒險總覽"
    },
    "empty": {
      "noAdventures": "暫無羈絆冒險"
    },
    "zoom": {
      "zoomIn": "放大",
      "zoomOut": "縮小",
      "reset": "重置"
    },
    "node": {
      "recommendStart": "推薦開始："
    },
    "status": {
      "completed": "已完成",
      "inProgress": "進行中",
      "unlocked": "可遊玩",
      "locked": "未解鎖"
    },
    "unlock": {
      "autoUnlock": "自動解鎖",
      "chatCount": "對話{count}次",
      "prereqAdventure": "完成前置冒險",
      "achievement": "解鎖特定成就"
    }
  },
  "characterInfo": {
    "header": {
      "title": "{title} - 配置編輯",
      "subtitle": "修改角色的詳細設置"
    },
    "tabs": {
      "basic": "基本信息",
      "prompts": "提示詞",
      "visuals": "視覺效果",
      "clothes": "服裝",
      "pet": "桌寵",
      "voice": "語音設置"
    },
    "fields": {
      "aiName": "AI 名稱",
      "aiSubtitle": "AI 副標題",
      "userName": "用户名稱",
      "userSubtitle": "用户副標題",
      "title": "角色標題",
      "info": "角色介紹",
      "systemPrompt": "系統提示詞",
      "systemPromptExample": "對話示例",
      "systemPromptExampleOld": "舊版兼容對話示例",
      "scale": "縮放",
      "offsetX": "水平偏移",
      "offsetY": "垂直偏移",
      "bubbleTop": "氣泡頂部距離",
      "bubbleLeft": "氣泡左側距離",
      "thinkingMessage": "思考消息文本",
      "scaleP": "桌寵縮放",
      "offsetXP": "桌寵水平偏移",
      "offsetYP": "桌寵垂直偏移",
      "ttsType": "TTS 類型",
      "voiceLang": "語音語言",
      "openttsVoice": "OpenTTS 音色標識"
    },
    "placeholders": {
      "openttsVoice": "留空則使用高級設置中的全局音色標識"
    },
    "voiceLangOptions": {
      "ja": "日語",
      "zh": "中文",
      "en": "英語",
      "ko": "韓語"
    },
    "clothes": {
      "listTitle": "服裝列表",
      "add": "添加服裝",
      "item": "服裝 #{index}",
      "empty": "暫無服裝配置，點擊\"添加服裝\"創建"
    },
    "footer": {
      "cancel": "取消",
      "save": "保存更改",
      "saving": "保存中..."
    },
    "messages": {
      "realtimeUpdateFailed": "實時更新 {label} 失敗，請檢查控制台日誌",
      "saveFailed": "保存失敗，請檢查控制台日誌"
    }
  },
  "characterCreate": {
    "header": {
      "title": "創建人物",
      "subtitle": "填寫設定並上傳頭像與 20 個情緒立繪"
    },
    "steps": {
      "basic": "基礎信息",
      "avatar": "立繪上傳",
      "advanced": "高級設置"
    },
    "form": {
      "resourceFolder": "角色目錄名",
      "resourceFolderPlaceholder": "例如: my_new_character",
      "title": "角色標題",
      "titlePlaceholder": "顯示在角色列表中的標題",
      "aiName": "AI 名稱",
      "aiNamePlaceholder": "角色對話名稱",
      "aiSubtitle": "AI 副標題",
      "aiSubtitlePlaceholder": "例如: 守夜人 / 學園偶像",
      "userName": "用户名稱",
      "userSubtitle": "用户副標題",
      "info": "角色簡介",
      "infoPlaceholder": "可選：用於角色介紹展示"
    },
    "avatar": {
      "uploadedStatus": "已上傳 {count}/20 情緒 + {avatar}",
      "avatarUploaded": "頭像已上傳",
      "avatarNotUploaded": "頭像未上傳",
      "missing": "缺少：{names}",
      "avatarLabel": "頭像",
      "uploaded": "已上傳",
      "notUploaded": "未上傳",
      "dropHint": "點擊或拖拽上傳"
    },
    "advanced": {
      "expand": "展開高級設置",
      "collapse": "收起高級設置",
      "scale": "縮放",
      "offset": "偏移",
      "bubbleTop": "氣泡頂部距離",
      "bubbleLeft": "氣泡左側距離",
      "thinkingMessage": "思考提示文本",
      "ttsType": "TTS 類型",
      "ttsNone": "不設置",
      "systemPrompt": "系統提示詞",
      "systemPromptExample": "對話示例",
      "systemPromptExampleOld": "舊版兼容示例"
    },
    "footer": {
      "prevStep": "上一步",
      "nextStep": "下一步",
      "creating": "創建中...",
      "confirmCreate": "確認創建"
    },
    "errors": {
      "basicIncomplete": "請先填寫目錄名、角色標題和 AI 名稱",
      "avatarIncomplete": "請上傳頭像和全部 20 個情緒立繪",
      "missingEmotionFile": "缺少情緒文件：{name}",
      "createFailed": "創建失敗"
    },
    "emotions": {
      "excited": "興奮",
      "disgusted": "厭惡",
      "crying": "哭泣",
      "scared": "害怕",
      "shy": "害羞",
      "calm": "平靜",
      "heartFlutter": "心動",
      "surprised": "驚訝",
      "flustered": "慌張",
      "worried": "擔心",
      "helpless": "無奈",
      "angry": "生氣",
      "confused": "疑惑",
      "nervous": "緊張",
      "confident": "自信",
      "serious": "認真",
      "playful": "調皮",
      "embarrassed": "難為情",
      "happy": "高興",
      "normal": "正常"
    }
  },
  "sceneEdit": {
    "title": {
      "create": "添加場景",
      "update": "更新場景"
    },
    "label": {
      "sceneName": "場景名稱",
      "sceneImage": "場景圖片",
      "sceneDescription": "場景描述",
      "glowColor": "發光色"
    },
    "placeholder": {
      "sceneName": "例如：海邊日落",
      "sceneDescription": "描述場景的環境、氛圍、光線等"
    },
    "option": {
      "selectBackground": "選擇背景圖片"
    },
    "button": {
      "upload": "上傳",
      "resetDefault": "重置為默認值",
      "cancel": "取消",
      "create": "創建",
      "update": "更新"
    },
    "lighting": {
      "title": "光影參數",
      "enableForScene": "為此場景啓用光影參數",
      "enableOverlay": "啓用光照疊加層"
    },
    "filter": {
      "character": "角色濾鏡",
      "background": "背景濾鏡"
    },
    "overlay": {
      "title": "光照疊加",
      "blend": "混合",
      "centerColor": "中心色",
      "edgeColor": "邊緣色",
      "target": "作用目標"
    },
    "overlayTarget": {
      "both": "角色 + 背景",
      "character": "僅角色",
      "background": "僅背景"
    },
    "slider": {
      "brightness": "亮度",
      "contrast": "對比度",
      "saturation": "飽和度",
      "sepia": "暖色調",
      "glowRadius": "發光半徑",
      "lightX": "光源 X",
      "lightY": "光源 Y",
      "overlayRadius": "光照半徑",
      "overlayOpacity": "光照強度"
    },
    "preview": {
      "title": "實時預覽",
      "placeholder": "選擇背景圖片後顯示預覽",
      "blendMode": "疊加混合: {mode}",
      "avatarLoaded": "已加載角色立繪"
    }
  },
  "background": {
    "scene": {
      "title": "場景管理",
      "current": "當前場景：",
      "none": "無",
      "create": "創建場景",
      "upload": "上傳背景",
      "openFolder": "打開文件夾",
      "delete": "刪除",
      "edit": "編輯場景",
      "noDescription": "暫無描述（選擇後不會觸發旁白）",
      "tip": "提示",
      "noDescriptionTip": "場景\"{name}\"暫無描述，選擇後不會觸發場景旁白",
      "deleteConfirm": "確定要刪除場景\"{name}\"嗎？"
    },
    "pagination": {
      "first": "← 首頁",
      "last": "末頁 →"
    },
    "particle": {
      "title": "粒子選擇",
      "none": "無",
      "starField": "星空",
      "rain": "雨水",
      "sakura": "櫻花",
      "snow": "雪景",
      "fireworks": "煙花"
    },
    "animation": {
      "switchTitle": "動畫開關",
      "settingsTitle": "動畫設置",
      "mainMenuStars": "啓用主界面星星粒子",
      "mainMenuMeteors": "啓用主界面流星動畫",
      "mouseTrail": "啓用全局鼠標滑動動畫",
      "clickAnimation": "啓用點擊動畫",
      "sceneAwareness": "啓用場景感知（切換場景時觸發旁白）",
      "meteorFps": "流星幀率 (FPS)",
      "starsFps": "星星幀率 (FPS)"
    },
    "cpu": {
      "title": "CPU 性能檢測",
      "detecting": "正在檢測 CPU …",
      "name": "CPU 名稱：",
      "tier": "性能等級：",
      "suggestedFps": "建議幀率：",
      "detectingShort": "檢測中…",
      "redetect": "重新檢測",
      "fetchFailed": "獲取 CPU 信息失敗",
      "detectComplete": "檢測完成",
      "tierMessage": "CPU 性能等級：{tier}",
      "redetectFailed": "重新檢測失敗"
    },
    "upload": {
      "invalidFormat": "請上傳支持的圖片格式: {formats}",
      "failed": "上傳失敗，請重試"
    },
    "folder": {
      "errorTitle": "錯誤",
      "openFailed": "無法打開文件夾"
    }
  },
  "text": {
    "font": {
      "title": "界面字體",
      "selectHint": "選擇界面顯示字體",
      "default": "軟件默認",
      "loading": "加載字體中…",
      "imported": "已導入",
      "importTitle": "導入字體文件 (.ttf / .woff2)",
      "demo": "字體演示 Font Sample 123"
    },
    "speed": {
      "title": "文字顯示速度",
      "label": "慢/快"
    },
    "sample": {
      "title": "顯示文字樣本",
      "demo": "Ling Chat: 測試文本顯示速度"
    },
    "inlineMotion": {
      "title": "內聯動作文本",
      "desc": "開啓後動作文本將與台詞同時顯示，無需二次點擊"
    },
    "sedentary": {
      "title": "久坐喝水提醒",
      "desc": "開啓後每40分鐘發送提醒一下久坐哦，只是健康小助手捏"
    },
    "memory": {
      "title": "啓用永久記憶",
      "desc": "開啓後記憶將會自動壓縮"
    },
    "voiceSound": {
      "title": "語音音效開關",
      "desc": "啓用無vits時的對話音效"
    },
    "engineDownload": {
      "title": "語音推理引擎下載（SBV2）",
      "cpuHint": "CPU 推理使用的是 SBV2-API，需要在 settings.yml 中把 sbv2 換成 sbv2api，人物設定也能改",
      "cpu": "CPU推理",
      "nvidia": "N卡推理",
      "amdHint": "A 卡推理使用的是 SBV2-API，需要在 settings.yml 中把 sbv2 換成 sbv2api，人物設定也能改",
      "amd": "A卡推理"
    },
    "back": {
      "title": "返回主菜單",
      "button": "返回主菜單",
      "refreshTts": "刷新TTS服務",
      "clearHistory": "清除歷史對話"
    },
    "ttsCache": {
      "title": "語音緩存",
      "current": "當前緩存",
      "files": "{count} 個文件",
      "lastCleanup": "最近已自動清理 {count} 個孤立語音文件",
      "orphan": "其中孤立文件 {count} 個（{size}）",
      "check": "檢查緩存",
      "clean": "清理孤立語音緩存",
      "cleanSuccess": "清理成功",
      "cleanDone": "清理完成",
      "cleanErrorTitle": "清理失敗",
      "cleanErrorMessage": "清理TTS緩存失敗",
      "unknown": "未知"
    },
    "update": {
      "title": "版本更新",
      "appVersion": "程序版本",
      "dataVersion": "數據版本",
      "checking": "檢查中...",
      "checkButton": "檢查程序更新",
      "downloading": "下載中...",
      "updateTo": "更新到 v{version}",
      "syncData": "同步數據",
      "statusChecking": "正在檢查更新...",
      "statusDownloading": "正在下載更新... {progress}%",
      "statusComplete": "更新完成，即將重啓...",
      "statusError": "檢查更新失敗",
      "statusAvailable": "發現新版本可用！"
    },
    "lanSync": {
      "title": "局域網數據同步",
      "desc": "在同一局域網內的設備之間同步 data 文件夾（遊戲存檔、語音、截圖等）。手機和電腦版互通必備~",
      "open": "打開局域網同步"
    },
    "docs": {
      "title": "瞭解 LingChat 的相關文檔",
      "desc": "如果你有任何疑惑，可以跳轉到這裏查看軟件的自定義玩法，問題解決，功能列表！",
      "button": "查看文檔"
    },
    "clearHistory": {
      "confirm": "清除歷史對話將丟失當前所有對話記錄，建議先存檔。\n\n是否已存檔或確認清除？",
      "successTitle": "清除成功",
      "successMessage": "對話歷史已清除",
      "errorTitle": "清除失敗",
      "errorMessage": "清除歷史對話失敗"
    },
    "refreshTts": {
      "success": "刷新TTS成功，將會在TTS可用的時候自動調用",
      "error": "刷新TTS失敗"
    }
  },
  "sound": {
    "volume": {
      "character": "角色音量",
      "bubble": "氣泡音量",
      "background": "背景音量",
      "achievement": "成就音量",
      "ambient": "環境音音量"
    },
    "slider": {
      "weakStrong": "弱/強"
    },
    "test": {
      "title": "聲音測試",
      "character": "測試角色",
      "bubble": "測試氣泡",
      "achievement": "測試成就"
    },
    "bgm": {
      "title": "背景音樂設置",
      "stop": "停止",
      "empty": "暫無音樂，請在下方上傳",
      "add": "添加音樂",
      "play": "播放",
      "pause": "暫停",
      "noMusicSelected": "未選擇音樂",
      "mode": {
        "loopList": "列表循環",
        "loopSingle": "單曲循環",
        "random": "隨機播放"
      },
      "selectFilesFirst": "請先選擇音樂文件",
      "confirmDelete": "確定要刪除《{name}》嗎？",
      "deleteFailed": "刪除音樂失敗",
      "uploadFailed": "部分或全部音樂上傳失敗"
    },
    "ambient": {
      "title": "環境音管理",
      "importedFiles": "已導入文件",
      "fileCount": "{count} 個",
      "empty": "暫無環境音文件，請在下方上傳",
      "addToTrack": "添加到播放軌道",
      "play": "播放",
      "add": "添加環境音",
      "playing": "播放中的環境音",
      "noPlaying": "暫無正在播放的環境音（從上方添加或通過劇本指令觸發）",
      "resume": "恢復播放",
      "pause": "暫停",
      "removeTrack": "移除軌道",
      "volumeLabel": "音量",
      "stopAll": "全部停止",
      "unknownName": "未知",
      "selectFilesFirst": "請先選擇環境音文件",
      "confirmDelete": "確定要刪除環境音《{name}》嗎？",
      "deleteFailed": "刪除環境音失敗",
      "uploadFailed": "部分或全部環境音上傳失敗"
    },
    "common": {
      "delete": "刪除",
      "selectedCount": "已選 {count} 個文件",
      "noSelection": "未選擇文件",
      "confirmUpload": "確認上傳",
      "unsupportedFormat": "格式不支持: {name}"
    }
  },
  "llmProviders": {
    "list": {
      "title": "已配置的模型",
      "restartApp": "重啓軟件",
      "addModel": "添加模型",
      "empty": "暫無配置的模型，點擊\"添加模型\"開始配置",
      "unnamed": "(未命名)",
      "modelNotSet": "未設置模型"
    },
    "role": {
      "chat": "對話",
      "translate": "翻譯",
      "vision": "視覺",
      "chatModel": "對話模型",
      "translateModel": "翻譯模型",
      "godAgent": "上帝Agent",
      "visionModel": "視覺模型",
      "notSelected": "未選擇",
      "followChat": "跟隨對話模型"
    },
    "action": {
      "edit": "編輯",
      "test": "測試",
      "delete": "刪除"
    },
    "panel": {
      "backToList": "返回模型列表",
      "editTitle": "編輯模型",
      "addTitle": "添加模型",
      "testTitle": "測試 {name}"
    },
    "form": {
      "presets": "預設（快速配置）",
      "label": "名稱",
      "labelPlaceholder": "例如: DeepSeek V3",
      "providerType": "提供商類型",
      "providerOpenai": "OpenAI 兼容",
      "providerLmstudio": "LM Studio（本地）",
      "modelName": "模型名稱",
      "modelPlaceholderLmstudio": "如: llama-3.2-3b-instruct",
      "modelPlaceholderDefault": "如: gpt-4o",
      "fetchModels": "自動獲取",
      "fetchingModels": "獲取中...",
      "reasoningEffort": "推理深度（部分模型支持）",
      "reasoningDefault": "默認（跟隨模型）",
      "effortLow": "Low（低）",
      "effortMedium": "Medium（中）",
      "effortHigh": "High（高）",
      "effortMax": "Max（最強）",
      "apiKey": "API 密鑰",
      "baseUrl": "API 地址",
      "temperature": "Temperature（留空使用默認）",
      "topP": "Top P（留空使用默認）",
      "enableThinking": "啓用思考鏈（部分模型支持）",
      "save": "保存",
      "cancel": "取消"
    },
    "test": {
      "placeholder": "輸入測試消息...",
      "testing": "測試中...",
      "send": "發送",
      "waiting": "等待響應...",
      "hint": "輸入消息並點擊發送，測試模型響應"
    },
    "msg": {
      "confirmDelete": "確定刪除模型 \"{name}\"？",
      "deleted": "已刪除",
      "deleteFailed": "刪除失敗: {error}",
      "saveSuccess": "保存成功！",
      "saveFailed": "保存失敗: {error}",
      "apiKeyRequired": "請先填寫 API 密鑰",
      "modelsFetched": "已獲取 {count} 個可用模型",
      "fetchFailed": "獲取失敗: {error}",
      "chatSwitched": "對話模型已切換並生效！",
      "translateSwitched": "翻譯模型已切換並生效！",
      "godAgentSwitched": "上帝Agent已切換並生效！",
      "switchFailed": "切換失敗: {error}"
    }
  }
}
