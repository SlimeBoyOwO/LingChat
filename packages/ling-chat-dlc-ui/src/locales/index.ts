export const extensionMessages: Record<string, Record<string, unknown>> = {
  "zh-CN": {
    views: {
      dlc: {
        storyEntry: "启用后，从开始游戏 → 剧情模式进入。",
        playEmpty: "还没有已启用的小游戏。请到设置 → 插件 / DLC → DLC 下载并启用。",
        unsupported: "此设备暂不支持主窗口内的小游戏。请使用桌面版。",
        title: "DLC",
        subtitle: "下载小游戏和剧本，按需更新。",
        import: "导入本地包",
        source: "内容目录地址",
        sync: "同步目录",
        updateAll: "更新已安装",
        working: "正在处理…",
        empty: "同步目录或导入本地包，开始添加 DLC。",
        version: "版本",
        installed: "已安装",
        install: "下载并安装",
        update: "更新",
        disable: "停用",
        enable: "启用",
        play: "开始",
        uninstall: "卸载",
        migrate: "迁移旧版数据",
        saveNote: "升级和卸载保留游戏存档；新安装的 DLC 需要先启用。",
        synced: "目录已同步。",
        installedNote: "安装完成。首次安装请先启用。",
        updated: "已安装的 DLC 已更新。",
        uninstallConfirm: "卸载这个 DLC？游戏存档会保留。",
        migrateConfirm:
          "把以下旧版浏览器数据复制到 DLC 存档？仅在新存档不存在时写入，原数据会保留。",
        noLegacy: "没有找到旧版数据。",
        migrated: "迁移完成；已有 DLC 存档会保留。",
      },
      mainMenu: {
        dlcRecognized: "已识别 DLC：{names}",
      },
      menu: {
        storyMode: "剧情模式",
        miniGame: "小游戏",
        resetMemoryTitle: "重置记忆",
        resetMemory: "重置记忆",
        resetMemoryDone2: "已重置",
        resetMemoryMessage:
          "将清除《{name}》的全部记忆（周目进度），恢复该剧本声明的安全剧情文件并清除它的主菜单效果；下次进入会从第一周目重新开始。\n\n确定要重置吗？",
        resetMemoryDone: "记忆已清除。她已经不记得你了。",
        resetMemoryEmpty: "这个剧本还没有任何记忆。",
        resetMemoryFailed: "重置失败，请稍后再试。",
      },
      contentWarning: {
        horrorTitle: "⚠ 内容警告",
        horrorMessage:
          "本剧本包含恐怖、惊吓与心理不适元素，以及强烈的画面崩坏演出，建议 16 岁及以上玩家游玩。\n部分恐怖剧本还会创建受限的本地故障窗口、在主窗口聚焦时短暂牵引系统鼠标（按 Esc 可立即停止），并在应用 data 目录旁的 characters 子目录中写入自身无害的 .chr 剧情标记；不会自动点击、执行外部命令或改动真实角色文件。\n如在游玩过程中感到不适，请立即退出。\n\n确定要继续吗？",
      },
    },
    settings: {
      plugins: {
        title: "插件 / DLC",
        tabs: {
          plugins: "已安装插件",
          dlc: "DLC",
        },
      },
    },
    nav: {
      plugins: "插件 / DLC",
    },
    ui: {
      characterCard: {
        switchFailed: "切换角色失败：{error}",
      },
    },
  },
  "zh-HK": {
    views: {
      dlc: {
        storyEntry: "啟用後，從開始遊戲 → 劇情模式進入。",
        playEmpty: "尚無已啟用的小遊戲。請到設定 → 插件 / DLC → DLC 下載並啟用。",
        unsupported: "此裝置暫不支援主視窗內的小遊戲。請使用桌面版。",
        title: "DLC",
        subtitle: "下載小遊戲與劇本，按需更新。",
        import: "匯入本機套件",
        source: "內容目錄網址",
        sync: "同步目錄",
        updateAll: "更新已安裝",
        working: "處理中…",
        empty: "同步目錄或匯入本機套件，開始新增 DLC。",
        version: "版本",
        installed: "已安裝",
        install: "下載並安裝",
        update: "更新",
        disable: "停用",
        enable: "啟用",
        play: "開始",
        uninstall: "解除安裝",
        migrate: "遷移舊版資料",
        saveNote: "升級及解除安裝會保留遊戲存檔；新安裝的 DLC 需要先啟用。",
        synced: "目錄已同步。",
        installedNote: "安裝完成。首次安裝請先啟用。",
        updated: "已安裝的 DLC 已更新。",
        uninstallConfirm: "解除安裝這個 DLC？遊戲存檔會保留。",
        migrateConfirm:
          "將以下舊版瀏覽器資料複製到 DLC 存檔？只會在新存檔不存在時寫入，原資料會保留。",
        noLegacy: "找不到舊版資料。",
        migrated: "遷移完成；已有 DLC 存檔會保留。",
      },
      mainMenu: {
        dlcRecognized: "已識別 DLC：{names}",
      },
      menu: {
        miniGame: "小遊戲",
        resetMemoryTitle: "重置記憶",
        resetMemory: "重置記憶",
        resetMemoryDone2: "已重置",
        resetMemoryMessage:
          "將清除《{name}》嘅全部記憶（周目進度），恢復該劇本已聲明嘅安全劇情檔案並清除佢嘅主選單效果；下次進入會由第一週目重新開始。\n\n確定要重置嗎？",
        resetMemoryDone: "記憶已清除。佢已經唔記得你喇。",
        resetMemoryEmpty: "呢個劇本仲未有任何記憶。",
        resetMemoryFailed: "重置失敗，請遲啲再試。",
      },
      contentWarning: {
        horrorTitle: "⚠ 內容警告",
        horrorMessage:
          "本劇本包含恐怖、驚嚇同心理不適元素，以及強烈嘅畫面崩壞演出，建議 16 歲或以上玩家遊玩。\n部分恐怖劇本亦會建立受限嘅本機故障視窗、喺主視窗聚焦時短暫牽引系統滑鼠（按 Esc 可即時停止），並喺應用程式 data 目錄旁邊嘅 characters 子目錄寫入自身無害嘅 .chr 劇情標記；唔會自動點擊、執行外部指令或改動真實角色檔案。\n如果遊玩期間感到不適，請立即退出。\n\n確定要繼續嗎？",
      },
    },
    settings: {
      plugins: {
        title: "插件 / DLC",
        tabs: {
          plugins: "已安裝插件",
          dlc: "DLC",
        },
      },
    },
    nav: {
      plugins: "插件 / DLC",
    },
    ui: {
      characterCard: {
        switchFailed: "切換角色失敗：{error}",
      },
    },
  },
  ja: {
    views: {
      dlc: {
        storyEntry: "有効にした後、ゲーム開始 → ストーリーモードから遊べます。",
        playEmpty:
          "有効なゲームがありません。設定 → プラグイン / DLC → DLC でダウンロードして有効にしてください。",
        unsupported:
          "この端末ではメイン画面内のゲームに未対応です。デスクトップ版をご利用ください。",
        title: "DLC",
        subtitle: "ミニゲームやシナリオをダウンロードして更新できます。",
        import: "パッケージを読み込む",
        source: "カタログ URL",
        sync: "カタログを同期",
        updateAll: "導入済みを更新",
        working: "処理中…",
        empty: "カタログを同期するか、ローカル DLC をインポートしてください。",
        version: "バージョン",
        installed: "導入済み",
        install: "ダウンロードして導入",
        update: "更新",
        disable: "無効化",
        enable: "有効化",
        play: "プレイ",
        uninstall: "アンインストール",
        migrate: "旧データを移行",
        saveNote: "更新・削除してもセーブデータは残ります。新規 DLC は有効化してください。",
        synced: "カタログを同期しました。",
        installedNote: "導入しました。新規 DLC を有効化してください。",
        updated: "導入済み DLC を更新しました。",
        uninstallConfirm: "この DLC を削除しますか？セーブデータは残ります。",
        migrateConfirm:
          "以下の旧ブラウザデータを DLC にコピーしますか？新しいセーブがない場合のみ書き込み、元のデータは残ります。",
        noLegacy: "旧データが見つかりません。",
        migrated: "移行しました。既存の DLC セーブは保持されます。",
      },
      mainMenu: {
        dlcRecognized: "DLC 認識済み：{names}",
      },
      menu: {
        miniGame: "ミニゲーム",
        resetMemoryTitle: "記憶をリセット",
        resetMemory: "記憶をリセット",
        resetMemoryDone2: "リセット済み",
        resetMemoryMessage:
          "《{name}》のすべての記憶（周回進行）を消去し、宣言済みの安全な物語用ファイルを復元して、このシナリオのメインメニュー効果を解除します。次回は1周目から始まります。\n\n本当にリセットしますか？",
        resetMemoryDone: "記憶を消去しました。彼女はもうあなたを覚えていません。",
        resetMemoryEmpty: "このシナリオにはまだ記憶がありません。",
        resetMemoryFailed: "リセットに失敗しました。後でもう一度お試しください。",
      },
      contentWarning: {
        horrorTitle: "⚠ コンテンツ警告",
        horrorMessage:
          "このシナリオにはホラー・驚かせる演出・精神的に不快な内容、および強烈な画面崩壊演出が含まれます。16歳以上のプレイヤーに推奨します。\n一部のホラーシナリオは、制限されたローカルのグリッチウィンドウを表示し、メインウィンドウにフォーカスがある間だけシステムポインターを短時間引き寄せ（Esc で即時停止）、characters サブディレクトリへ無害な .chr 物語マーカーを書き込みます。自動クリック、外部コマンドの実行、実際のキャラクターファイルの変更は行いません。\nプレイ中に気分が悪くなった場合は、すぐに終了してください。\n\n本当に続けますか？",
      },
    },
    settings: {
      plugins: {
        title: "プラグイン / DLC",
        tabs: {
          plugins: "インストール済みプラグイン",
          dlc: "DLC",
        },
      },
    },
    nav: {
      plugins: "プラグイン / DLC",
    },
    ui: {
      characterCard: {
        switchFailed: "キャラ切替に失敗しました：{error}",
      },
    },
  },
  en: {
    views: {
      dlc: {
        storyEntry: "After enabling, open Start Game → Story Mode.",
        playEmpty:
          "No games are enabled. Download and enable them in Settings → Plugins / DLC → DLC.",
        unsupported:
          "Embedded games are not supported on this device yet. Please use the desktop app.",
        title: "DLC",
        subtitle: "Download and update games and stories.",
        import: "Import package",
        source: "Catalog URL",
        sync: "Sync catalog",
        updateAll: "Update installed",
        working: "Working…",
        empty: "Sync the catalog or import a local DLC package.",
        version: "Version",
        installed: "Installed",
        install: "Download & install",
        update: "Update",
        disable: "Disable",
        enable: "Enable",
        play: "Play",
        uninstall: "Uninstall",
        migrate: "Migrate legacy data",
        saveNote:
          "Updates and uninstall keep game saves. Enable newly installed DLC before playing.",
        synced: "Catalog synchronized.",
        installedNote: "Installed. Enable newly installed DLC to play.",
        updated: "Installed DLC updated.",
        uninstallConfirm: "Uninstall this DLC? Game saves will be kept.",
        migrateConfirm:
          "Copy these legacy browser keys into the DLC save? Only a missing save is initialized; original data is kept.",
        noLegacy: "No legacy data found.",
        migrated: "Migration complete; existing DLC saves are kept.",
      },
      mainMenu: {
        dlcRecognized: "DLC detected: {names}",
      },
      menu: {
        miniGame: "Mini Games",
        resetMemoryTitle: "Reset Memory",
        resetMemory: "Reset Memory",
        resetMemoryDone2: "Reset",
        resetMemoryMessage:
          'This will erase all memories (playthrough progress) of "{name}", restore its declared safe narrative files, and clear its main-menu effect. Your next visit will start from the first playthrough.\n\nAre you sure?',
        resetMemoryDone: "Memories erased. She no longer remembers you.",
        resetMemoryEmpty: "This script has no stored memories yet.",
        resetMemoryFailed: "Reset failed. Please try again later.",
      },
      contentWarning: {
        horrorTitle: "⚠ Content Warning",
        horrorMessage:
          "This story contains horror, jumpscares and intense visual corruption effects. Recommended for players aged 16 and above.\nSome horror scripts also create restricted local glitch windows, briefly pull the system pointer while the main window is focused (press Esc to stop), and write harmless .chr narrative markers under the characters folder beside the app data directory. They never click automatically, run external commands, or modify real character files.\nIf you feel unwell while playing, please quit immediately.\n\nAre you sure you want to continue?",
      },
    },
    settings: {
      plugins: {
        title: "Plugins / DLC",
        tabs: {
          plugins: "Installed plugins",
          dlc: "DLC",
        },
      },
    },
    nav: {
      plugins: "Plugins / DLC",
    },
    ui: {
      characterCard: {
        switchFailed: "Failed to switch character: {error}",
      },
    },
  },
};
