// 繁體中文（香港 · 粵語文體）语言包，由 zh-CN/stores.ts（约 22 条） 翻譯維護
export default {
  "dialog": {
    "alertTitle": "提示",
    "confirmTitle": "確認"
  },
  "notification": {
    "errorTitle": "錯誤",
    "unknownError": "發生咗未知錯誤",
    "switchSuccessTitle": "切換成功",
    "switchFailTitle": "切換失敗",
    "switchSuccessMessage": "角色已經切換咗",
    "switchFailMessage": "切換嗰陣出咗問題",
    "refreshSuccessTitle": "重新整理成功",
    "refreshFailTitle": "重新整理失敗",
    "refreshSuccessMessage": "角色列表已經重新整理好喇！",
    "refreshFailMessage": "重新整理嗰陣出咗問題"
  },
  "llmErrors": {
    "invalid_api_key": "笨蛋，API Key 填錯啦！請檢查 API Key 是否正確、有冇多餘空格、係咪過期或者額度用盡。",
    "forbidden": "存取被拒絕（403）：請檢查 API Key 對呢個介面有冇權限。",
    "not_found": "搵唔到介面或模型（404）：請檢查 Base URL 路徑啱唔啱、模型名係咪真係存在。",
    "rate_limited": "請求太密（429）：請稍後再試，或者檢查係咪超出額度/頻率限制。",
    "server_error": "服務商伺服器錯誤（5xx）：請稍後再試，或者睇下服務商嘅服務狀態。",
    "timeout": "請求超時：請檢查網絡係咪穩定、服務商係咪繁忙，或者調大「LLM 請求空閒超時」設定。",
    "network_error": "網絡連線失敗：請檢查網絡係咪斷開、Base URL 地址係咪可訪問、需唔需要代理。",
    "empty_response": "模型返咗空內容：請試下換個模型，或者檢查輸入參數。",
    "invalid_config": "配置唔完整：請檢查 API Key 同模型名。",
    "other": "發生未預期嘅錯誤，請檢查配置後再試。"
  },
  "zoom": {
    "toastTitle": "縮放"
  },
  "updater": {
    "networkError": "網絡連線失敗，上唔到更新伺服器。唔該檢查下網絡之後再試過。",
    "noUpdateAvailable": "冇可用嘅更新"
  },
  "sedentaryReminder": {
    "notificationTitle": "LingChat 久坐提提你",
    "notificationBody": "坐咗有啲耐，記得郁一郁喎"
  },
  "lanSync": {
    "noPeerSelected": "未揀對等裝置",
    "manualRestart": "請手動重新啟動應用程式，先可以套用同步檔案",
    "reason": {
      "new": "新增",
      "modified": "修改",
      "newer": "更新"
    }
  }
}
