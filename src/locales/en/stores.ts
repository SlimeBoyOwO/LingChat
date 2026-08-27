// English language pack, maintained from zh-CN/stores.ts
export default {
  dialog: {
    alertTitle: "Notice",
    confirmTitle: "Confirm",
  },
  notification: {
    errorTitle: "Error",
    unknownError: "An unknown error occurred",
    switchSuccessTitle: "Switched Successfully",
    switchFailTitle: "Switch Failed",
    switchSuccessMessage: "Character switched",
    switchFailMessage: "Something went wrong while switching",
    refreshSuccessTitle: "Refreshed Successfully",
    refreshFailTitle: "Refresh Failed",
    refreshSuccessMessage: "Character list refreshed successfully!",
    refreshFailMessage: "Something went wrong while refreshing",
  },
  llmErrors: {
    invalid_api_key: "Your API key is invalid! Please check it for typos or extra spaces, and whether it has expired or run out of quota.",
    forbidden: "Access denied (403): check that your API key has permission for this endpoint.",
    not_found: "Endpoint or model not found (404): check the Base URL path and the model name.",
    rate_limited: "Too many requests (429): try again later, or check quota/rate limits.",
    server_error: "Provider server error (5xx): try again later, or check provider status.",
    timeout: "Request timed out: check network stability, provider load, or increase the idle timeout setting.",
    network_error: "Network connection failed: check your network, Base URL accessibility, or proxy.",
    empty_response: "The model returned an empty response: try a different model or adjust parameters.",
    invalid_config: "Incomplete configuration: please check the API key and model name.",
    other: "An unexpected error occurred. Check your configuration and retry.",
  },
  zoom: {
    toastTitle: "Zoom",
  },
  updater: {
    networkError: "Network connection failed — can't reach the update server. Please check your network and try again.",
    noUpdateAvailable: "No updates available",
  },
  sedentaryReminder: {
    notificationTitle: "LingChat Sedentary Reminder",
    notificationBody: "You've been sitting for a while — time to stretch a little!",
  },
  lanSync: {
    noPeerSelected: "No peer device selected",
    manualRestart: "Please restart the app manually to apply the synced files",
    reason: {
      new: "Added",
      modified: "Modified",
      newer: "Updated",
    },
  },
}
