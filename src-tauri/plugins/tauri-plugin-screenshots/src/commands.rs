use std::{
    fs::{create_dir_all, remove_dir_all, remove_file},
    path::PathBuf,
};

use serde::Serialize;
use tauri::{command, AppHandle, Manager, Runtime};
use xcap::{Monitor, Window};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotableWindow {
    pub id: u32,
    pub name: String,
    pub title: String,
    pub app_name: String,
}

#[derive(Debug, Serialize)]
pub struct ScreenshotableMonitor {
    pub id: u32,
    pub name: String,
}

/// Get all windows that can take screenshots.
///
/// # Returns
/// - `Ok(Vec<ScreenshotableWindow>)`: List of screenshotable window information.
/// - `Err(String)`: An error message string on failure.
///
/// # Example
/// ```
/// use tauri_plugin_screenshots::get_screenshotable_windows;
///
/// let windows = get_screenshotable_windows().await.unwrap();
/// println!("{:#?}", windows); // Vec<ScreenshotableWindow>
/// ```
#[command]
pub async fn get_screenshotable_windows() -> Result<Vec<ScreenshotableWindow>, String> {
    // On Windows, use our custom enumeration that does NOT filter out
    // own-process windows (fixes issue #8).
    #[cfg(target_os = "windows")]
    {
        crate::windows::enumerate_all_windows()
    }

    // Non-Windows: use xcap's Window::all()
    #[cfg(not(target_os = "windows"))]
    {
        let windows = Window::all().map_err(|err| err.to_string())?;

        let mut screenshotable_windows = vec![];

        for window in windows {
            if window.is_minimized().map_err(|err| err.to_string())? {
                continue;
            }

            let app_name = window.app_name().map_err(|err| err.to_string())?;
            let title = window.title().map_err(|err| err.to_string())?;

            let name = if title.is_empty() || app_name.eq(&title) {
                app_name.clone()
            } else {
                format!("{} - {}", app_name, title)
            };

            screenshotable_windows.push(ScreenshotableWindow {
                id: window.id().map_err(|err| err.to_string())?,
                name,
                title,
                app_name,
            });
        }

        Ok(screenshotable_windows)
    }
}

/// Get all monitors that can take screenshots.
///
/// # Returns
/// - `Ok(Vec<ScreenshotableMonitor>)`: List of screenshotable monitor information.
/// - `Err(String)`: An error message string on failure.
///
/// # Example
/// ```
/// use tauri_plugin_screenshots::get_screenshotable_monitors;
///
/// let monitors = get_screenshotable_monitors().await.unwrap();
/// println!("{:#?}", monitors); // Vec<ScreenshotableMonitor>
/// ```
#[command]
pub async fn get_screenshotable_monitors() -> Result<Vec<ScreenshotableMonitor>, String> {
    let monitors = Monitor::all().map_err(|err| err.to_string())?;

    let mut screenshotable_monitors = vec![];

    for monitor in monitors {
        screenshotable_monitors.push(ScreenshotableMonitor {
            id: monitor.id().map_err(|err| err.to_string())?,
            name: monitor.name().map_err(|err| err.to_string())?.to_string(),
        });
    }

    Ok(screenshotable_monitors)
}

fn get_save_dir<R: Runtime>(app_handle: AppHandle<R>) -> Result<PathBuf, String> {
    let save_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|err| err.to_string())?
        .join("tauri-plugin-screenshots");

    Ok(save_dir)
}

fn get_save_path<R: Runtime>(
    app_handle: AppHandle<R>,
    id: u32,
    is_window: bool,
) -> Result<PathBuf, String> {
    let prefix = if is_window { "window" } else { "monitor" };

    let save_dir = get_save_dir(app_handle)?;

    create_dir_all(&save_dir).map_err(|err| err.to_string())?;

    let save_path = save_dir.join(format!("{prefix}-{id}.png"));

    Ok(save_path)
}

/// Get a screenshot of the window with the specified id.
///
/// # Arguments
///
/// - `id`: Window id.
///
/// # Returns
/// - `Ok(PathBuf)`: Path to store the image.
/// - `Err(String)`: An error message string on failure.
///
/// # Example
/// ```
/// use tauri_plugin_screenshots::get_window_screenshot;
///
/// let path = get_window_screenshot(app_handle, 1).await.unwrap();
/// println!("{:?}", path); // xx/tauri-plugin-screenshots/window-1.png
/// ```
#[command]
pub async fn get_window_screenshot<R: Runtime>(
    app_handle: AppHandle<R>,
    id: u32,
) -> Result<PathBuf, String> {
    // First, try to find the window via xcap's Window::all().
    if let Ok(windows) = Window::all() {
        if let Some(window) = windows.iter().find(|item| {
            item.id().is_ok_and(|wid| wid == id)
        }) {
            if window.is_minimized().map_err(|err| err.to_string())? {
                return Err("Minimized windows can't take screenshots".to_string());
            }

            let image = window.capture_image().map_err(|err| err.to_string())?;
            let save_path = get_save_path(app_handle, id, true)?;
            image.save(&save_path).map_err(|err| err.to_string())?;
            return Ok(save_path);
        }
    }

    // On Windows: if xcap didn't find the window, it might be an own-process
    // window that xcap's PID filter excluded. Try capturing it directly.
    #[cfg(target_os = "windows")]
    {
        if crate::windows::is_window_minimized(id) {
            return Err("Minimized windows can't take screenshots".to_string());
        }

        let image = crate::windows::capture_own_window(id)?;
        let save_path = get_save_path(app_handle, id, true)?;
        image.save(&save_path).map_err(|err| err.to_string())?;
        Ok(save_path)
    }

    #[cfg(not(target_os = "windows"))]
    Err("Window not found".to_string())
}

/// Get a screenshot of the monitor with the specified id.
///
/// # Arguments
///
/// - `id`: Monitor id.
///
/// # Returns
/// - `Ok(PathBuf)`: Path to store the image.
/// - `Err(String)`: An error message string on failure.
///
/// # Example
/// ```
/// use tauri_plugin_screenshots::get_monitor_screenshot;
///
/// let path = get_monitor_screenshot(app_handle, 1).await.unwrap();
/// println!("{:?}", path); // xx/tauri-plugin-screenshots/monitor-1.png
/// ```
#[command]
pub async fn get_monitor_screenshot<R: Runtime>(
    app_handle: AppHandle<R>,
    id: u32,
) -> Result<PathBuf, String> {
    let monitors = Monitor::all().map_err(|err| err.to_string())?;

    if let Some(monitor) = monitors.iter().find(|item| item.id().map_err(|err| err.to_string()).ok() == Some(id)) {
        let image = monitor.capture_image().map_err(|err| err.to_string())?;

        let save_path = get_save_path(app_handle, monitor.id().map_err(|err| err.to_string())?, false)?;

        image.save(&save_path).map_err(|err| err.to_string())?;

        return Ok(save_path);
    };

    Err("Monitor not found".to_string())
}

/// Remove locally stored window screenshots.
///
/// # Arguments
///
/// - `id`: Window id.
///
/// # Returns
/// - `Ok(())`: Success.
/// - `Err(String)`: An error message string on failure.
///
/// # Example
/// ```
/// use tauri_plugin_screenshots::remove_window_screenshot;
///
/// remove_window_screenshot(app_handle, 1).await.unwrap();
/// ```
#[command]
pub async fn remove_window_screenshot<R: Runtime>(
    app_handle: AppHandle<R>,
    id: u32,
) -> Result<(), String> {
    let path = get_save_path(app_handle, id, true)?;

    remove_file(path).map_err(|err| err.to_string())
}

/// Remove locally stored monitor screenshots.
///
/// # Arguments
///
/// - `id`: Monitor id.
///
/// # Returns
/// - `Ok(())`: Success.
/// - `Err(String)`: An error message string on failure.
///
/// # Example
/// ```
/// use tauri_plugin_screenshots::remove_monitor_screenshot;
///
/// remove_monitor_screenshot(app_handle, 1).await.unwrap();
/// ```
#[command]
pub async fn remove_monitor_screenshot<R: Runtime>(
    app_handle: AppHandle<R>,
    id: u32,
) -> Result<(), String> {
    let path = get_save_path(app_handle, id, false)?;

    remove_file(path).map_err(|err| err.to_string())
}

/// Remove all locally stored screenshots.
///
/// # Returns
/// - `Ok(())`: Success.
/// - `Err(String)`: An error message string on failure.
///
/// # Example
/// ```
/// use tauri_plugin_screenshots::clear_screenshots;
///
/// clear_screenshots(app_handle).await.unwrap();
#[command]
pub async fn clear_screenshots<R: Runtime>(app_handle: AppHandle<R>) -> Result<(), String> {
    let save_dir = get_save_dir(app_handle)?;

    remove_dir_all(save_dir).map_err(|err| err.to_string())
}