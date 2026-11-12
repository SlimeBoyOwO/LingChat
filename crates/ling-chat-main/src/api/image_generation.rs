use crate::AppState;
use crate::ai_service::image_generation::{
    self, GenerateImageRequest, GeneratedImage, ImageGenerationSettings, ImageModelCatalog,
    ImageProviderInfo,
};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub fn list_image_generation_providers(app: AppHandle) -> Vec<ImageProviderInfo> {
    image_generation::list_providers(&app)
}

#[tauri::command]
pub async fn list_image_generation_models(
    app: AppHandle,
    mut settings: ImageGenerationSettings,
    refresh: bool,
) -> Result<ImageModelCatalog, String> {
    settings.normalize();
    image_generation::list_models(&app, &settings, refresh)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_image_generation_settings(
    app: AppHandle,
    mut settings: ImageGenerationSettings,
) -> Result<(), String> {
    settings.normalize();
    let mut tools = app.state::<AppState>().tool_settings.get();
    tools.image_generation = settings;
    super::tool_settings::save_tool_settings(app, tools).await
}

#[tauri::command]
pub async fn generate_image(
    app: AppHandle,
    request: GenerateImageRequest,
) -> Result<GeneratedImage, String> {
    image_generation::generate(&app, request)
        .await
        .map_err(|e| e.to_string())
}
