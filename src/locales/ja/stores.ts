export default {
  dialog: {
    alertTitle: 'お知らせ',
    confirmTitle: '確認',
  },
  notification: {
    errorTitle: 'エラー',
    unknownError: '不明なエラーが発生しました',
    switchSuccessTitle: '切り替え成功',
    switchFailTitle: '切り替え失敗',
    switchSuccessMessage: 'キャラクターを切り替えました',
    switchFailMessage: '切り替え中に問題が発生しました',
    refreshSuccessTitle: '更新成功',
    refreshFailTitle: '更新失敗',
    refreshSuccessMessage: 'キャラクターリストを更新しました！',
    refreshFailMessage: '更新中に問題が発生しました',
  },
  llmErrors: {
    invalid_api_key: 'API キーが正しくありません！API キーの入力（余分なスペース含む）、期限切れ、残高不足を確認してください。',
    forbidden: 'アクセスが拒否されました（403）：このエンドポイントに対する権限を確認してください。',
    not_found: 'エンドポイントまたはモデルが見つかりません（404）：Base URL のパスとモデル名を確認してください。',
    rate_limited: 'リクエストが多すぎます（429）：時間をおいて再試行するか、回数・残高制限を確認してください。',
    server_error: 'プロバイダのサーバーエラー（5xx）：時間をおいて再試行するか、プロバイダの稼働状況を確認してください。',
    timeout: 'リクエストがタイムアウトしました：ネットワーク状況、プロバイダの負荷、タイムアウト設定を確認してください。',
    network_error: 'ネットワーク接続に失敗しました：ネットワーク、Base URL の到達可能性、プロキシを確認してください。',
    empty_response: 'モデルが空の応答を返しました：別のモデルを試すか、パラメータを確認してください。',
    invalid_config: '設定が不完全です：API キーとモデル名を確認してください。',
    other: '予期しないエラーが発生しました。設定を確認して再試行してください。',
  },
  zoom: {
    toastTitle: 'ズーム',
  },
  updater: {
    networkError:
      'ネットワーク接続に失敗しました。更新サーバーにアクセスできません。ネットワークを確認してからもう一度お試しください。',
    noUpdateAvailable: '利用可能な更新はありません',
  },
  sedentaryReminder: {
    notificationTitle: 'LingChat 久坐リマインダー',
    notificationBody: '長時間座りっぱなしです。少し体を動かしましょう',
  },
  lanSync: {
    noPeerSelected: '同期先のデバイスが選択されていません',
    manualRestart: '同期ファイルを適用するには、アプリを手動で再起動してください',
    reason: {
      new: '新規',
      modified: '変更',
      newer: '更新',
    },
  },
}
