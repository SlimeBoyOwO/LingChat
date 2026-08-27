export default {
  dialog: {
    alertTitle: '提示',
    confirmTitle: '确认',
  },
  notification: {
    errorTitle: '错误',
    unknownError: '发生了未知错误',
    switchSuccessTitle: '切换成功',
    switchFailTitle: '切换失败',
    switchSuccessMessage: '角色已切换',
    switchFailMessage: '切换时出了问题',
    refreshSuccessTitle: '刷新成功',
    refreshFailTitle: '刷新失败',
    refreshSuccessMessage: '角色列表已成功刷新！',
    refreshFailMessage: '刷新时出了问题',
  },
  llmErrors: {
    invalid_api_key: '笨蛋，API Key 填错啦！请检查 API Key 是否正确、有无多余空格、是否过期或额度已用尽。',
    forbidden: '访问被拒绝（403）：请检查 API Key 是否对该接口有权限，或是否被服务商封禁。',
    not_found: '找不到接口或模型（404）：请检查 Base URL 路径是否正确、模型名是否真实存在。',
    rate_limited: '请求过于频繁（429）：请稍后再试，或检查是否触发了额度/频率限制。',
    server_error: '服务商服务器错误（5xx）：请稍后再试，或查看服务商的服务状态。',
    timeout: '请求超时：请检查网络是否稳定、服务商是否繁忙，或调大「LLM 请求空闲超时」设置。',
    network_error: '网络连接失败：请检查网络是否断开、Base URL 地址是否可访问、是否需要代理。',
    empty_response: '模型返回了空内容：请尝试更换模型，或检查输入参数。',
    invalid_config: '配置不完整：请检查 API Key 和模型名称。',
    other: '发生未预期的错误，请检查配置后重试。',
  },
  zoom: {
    toastTitle: '缩放',
  },
  updater: {
    networkError: '网络连接失败，无法访问更新服务器。请检查网络后重试。',
    noUpdateAvailable: '没有可用的更新',
  },
  sedentaryReminder: {
    notificationTitle: 'LingChat 久坐提醒',
    notificationBody: '久坐时间有点长，记得活动一下哦',
  },
  lanSync: {
    noPeerSelected: '未选择对等设备',
    manualRestart: '请手动重启应用以应用同步文件',
    reason: {
      new: '新增',
      modified: '修改',
      newer: '更新',
    },
  },
}
