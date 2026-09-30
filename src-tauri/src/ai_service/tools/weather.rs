use async_trait::async_trait;
use once_cell::sync::Lazy;
use serde_json::{Value, json};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::ai_service::types::ToolDefinition;
use crate::config::AppConfig;

use super::executor::{Tool, ToolContext, ToolError, ToolResult};

/// ============ 天气上下文缓存 ============
///
/// 天气感知定位为"环境上下文"而非"问答工具"：模型应该随时知道外面什么天气
/// （像知道当前时间一样），而不是被问到才临时查。缓存在两处被填充——
/// query_weather 工具执行成功时、后台刷新循环（run_refresh_loop）定时拉取时；
/// 消息处理器（processor.rs）把未过期的摘要注入系统提醒。

const WEATHER_TTL: Duration = Duration::from_secs(30 * 60);

struct CachedWeather {
    summary: String,
    fetched_at: Instant,
}

static WEATHER_CACHE: Lazy<Mutex<Option<CachedWeather>>> = Lazy::new(|| Mutex::new(None));

/// 未过期的自然语言天气摘要，如「成都 小雨 21.5°C（体感 20°C）」。
pub fn cached_summary() -> Option<String> {
    let cache = WEATHER_CACHE.lock().ok()?;
    let cached = cache.as_ref()?;
    (cached.fetched_at.elapsed() <= WEATHER_TTL).then(|| cached.summary.clone())
}

fn store_summary(summary: String) {
    if let Ok(mut cache) = WEATHER_CACHE.lock() {
        *cache = Some(CachedWeather {
            summary,
            fetched_at: Instant::now(),
        });
    }
}

/// 读取配置里的手动城市（trim 后非空才视为有效）。
fn configured_city(config: &AppConfig) -> Option<String> {
    let city = config.weather_city.trim().to_string();
    (!city.is_empty()).then_some(city)
}

/// 后台刷新循环：每 5 分钟检查一次，缓存缺失/过期时重新拉取。城市来源优先级：
/// 手动配置的城市 > IP 自动定位（需开关打开）。由 lib.rs 在启动时 spawn。
pub(crate) async fn run_refresh_loop(app: tauri::AppHandle) {
    loop {
        let config = AppConfig::load(&app).ok();
        let city = config.as_ref().and_then(configured_city);
        let ip_enabled = config
            .as_ref()
            .map(|c| c.weather_ip_location)
            .unwrap_or(false);

        let stale = WEATHER_CACHE
            .lock()
            .ok()
            .and_then(|c| c.as_ref().map(|w| w.fetched_at.elapsed() > WEATHER_TTL))
            .unwrap_or(true);

        if stale {
            match reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
            {
                Ok(client) => {
                    // 三种来源都可能不适用（没配城市且 IP 关闭），用 Option 表达
                    let fetch = if let Some(city) = city {
                        Some(fetch_summary_for_city(&client, &city).await)
                    } else if ip_enabled {
                        Some(fetch_summary_via_ip(&client).await)
                    } else {
                        // 没配城市也没开 IP 定位：不刷新，等对话中说出城市后
                        // 由 query_weather 填充缓存
                        None
                    };
                    if let Some(result) = fetch {
                        match result {
                            Ok(summary) => store_summary(summary),
                            Err(e) => tracing::warn!("天气缓存刷新失败: {e}"),
                        }
                    }
                },
                Err(e) => tracing::warn!("天气客户端创建失败: {e}"),
            }
        }
        tokio::time::sleep(Duration::from_secs(5 * 60)).await;
    }
}

/// WMO weather interpretation code → 中文天气描述。
fn describe_weather_code(code: i64) -> &'static str {
    match code {
        0 => "晴",
        1 | 2 | 3 => "多云",
        45 | 48 => "雾",
        51..=57 => "毛毛雨",
        61..=67 => "雨",
        71..=77 => "雪",
        80..=82 => "阵雨",
        85 | 86 => "阵雪",
        95..=99 => "雷雨",
        _ => "未知",
    }
}

/// 给模型的氛围建议：映射到 set_background_effect 的特效名。
fn effect_hint_for(code: i64) -> &'static str {
    match code {
        51..=67 | 80..=82 | 95..=99 => "Rain",
        71..=77 | 85 | 86 => "Snow",
        45 | 48 => "none",
        0 | 1 => "StarField 或 none（夜晚适合 StarField）",
        _ => "none",
    }
}

/// 城市名 → (显示名, 纬度, 经度)，Open-Meteo Geocoding（免 key）。
async fn geocode_city(
    client: &reqwest::Client,
    city: &str,
) -> Result<(String, String, String), ToolError> {
    let geo: Value = client
        .get("https://geocoding-api.open-meteo.com/v1/search")
        .query(&[
            ("name", city),
            ("count", "1"),
            ("language", "zh"),
            ("format", "json"),
        ])
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| ToolError::Execution(format!("城市查询失败（请检查网络）: {e}")))?
        .json()
        .await
        .map_err(|e| ToolError::Execution(format!("解析城市数据失败: {e}")))?;

    let Some(location) = geo["results"].as_array().and_then(|a| a.first()) else {
        return Err(ToolError::Execution(format!("找不到城市「{city}」")));
    };
    Ok((
        location["name"].as_str().unwrap_or(city).to_string(),
        location["latitude"].to_string(),
        location["longitude"].to_string(),
    ))
}

/// IP 定位 → (中文城市名, 纬度, 经度)。
///
/// ip-api.com 免费端点（免 key，支持中文，返回经纬度）；免费版仅提供 http，
/// 本地原型可接受。挂在用户显式开启的开关之后，开启方式在设置页。
async fn locate_by_ip(client: &reqwest::Client) -> Result<(String, String, String), ToolError> {
    let ip: Value = client
        .get("http://ip-api.com/json/?lang=zh-CN&fields=status,city,lat,lon")
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| ToolError::Execution(format!("IP 定位失败: {e}。请自然地请用户直接说城市名")))?
        .json()
        .await
        .map_err(|e| ToolError::Execution(format!("解析 IP 定位数据失败: {e}")))?;

    if ip["status"].as_str() != Some("success") {
        return Err(ToolError::Execution(
            "IP 定位失败。请自然地请用户直接说城市名".into(),
        ));
    }
    Ok((
        ip["city"].as_str().unwrap_or("当前城市").to_string(),
        ip["lat"].to_string(),
        ip["lon"].to_string(),
    ))
}

/// 经纬度 → 当前天气 JSON（Open-Meteo Forecast）。
async fn fetch_current(client: &reqwest::Client, lat: &str, lon: &str) -> Result<Value, ToolError> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}\
         &current=temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,\
         wind_speed_10m&timezone=auto&forecast_days=1"
    );
    let weather: Value = client
        .get(&url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| ToolError::Execution(format!("天气查询失败（请检查网络）: {e}")))?
        .json()
        .await
        .map_err(|e| ToolError::Execution(format!("解析天气数据失败: {e}")))?;
    if weather["current"].is_null() {
        return Err(ToolError::Execution("天气数据为空".into()));
    }
    Ok(weather)
}

/// 城市名 → 自然语言摘要（geocode + 拉取 + 组装）。
async fn fetch_summary_for_city(client: &reqwest::Client, city: &str) -> Result<String, ToolError> {
    let (city, lat, lon) = geocode_city(client, city).await?;
    let weather = fetch_current(client, &lat, &lon).await?;
    Ok(format_summary(&city, &weather["current"]))
}

fn format_summary(city: &str, current: &Value) -> String {
    let code = current["weather_code"].as_i64().unwrap_or(-1);
    format!(
        "{} {} {}°C（体感 {}°C）",
        city,
        describe_weather_code(code),
        current["temperature_2m"],
        current["apparent_temperature"],
    )
}

/// IP 定位 → 自然语言摘要。
async fn fetch_summary_via_ip(client: &reqwest::Client) -> Result<String, ToolError> {
    let (city, lat, lon) = locate_by_ip(client).await?;
    let weather = fetch_current(client, &lat, &lon).await?;
    Ok(format_summary(&city, &weather["current"]))
}

/// query_weather：查询实时天气（Open-Meteo，免 API key）。
///
/// 城市解析两级策略：用户在对话中明确说出的城市优先；未说时若用户开启了
/// 「IP 自动定位」开关则走 IP 定位；两者皆无则提示模型自然地向用户询问。
pub struct WeatherTool;

#[async_trait]
impl Tool for WeatherTool {
    /// 外网 API 往返需要数秒，默认的 2 秒执行器超时必炸，放宽到 15 秒。
    fn timeout_hint(&self) -> Option<Duration> {
        Some(Duration::from_secs(15))
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "query_weather",
            "获取用户所在城市的实时天气。城市解析：用户在对话里说过的城市优先（填入 city）；\
             没说时不要向用户追问城市名，直接不带 city 调用——若用户开启了 IP 自动定位会自动定位，\
             未开启则会提示你询问。\
             返回结果只是给你自己看的参考——**绝不要在回复里罗列温度、湿度等原始数据**，\
             用一两句自然的口语把天气感受融进对话（例如「外面下着雨呢，出门记得带把伞」）。\
             拿到结果后可以顺手用 set_background_effect 切粒子特效（effect_hint 字段给了建议值）、\
             必要时用 scene_switch 换更贴合的背景、用 change_clothes 换身合适的衣服",
            json!({
                "type": "object",
                "properties": {
                    "city": {"type": "string", "description": "城市中文名；仅当用户在对话中明确说了城市时才填"}
                },
                "required": [],
                "additionalProperties": false
            }),
        )
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let Some(obj) = arguments.as_object() else {
            return Err(ToolError::InvalidArguments("参数必须是 JSON object".into()));
        };
        let stated_city = obj
            .get("city")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        let app = context.require_app()?;
        let config = AppConfig::load(&app).ok();
        let ip_enabled = config
            .as_ref()
            .map(|c| c.weather_ip_location)
            .unwrap_or(false);
        let configured_city = config.as_ref().and_then(configured_city);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| ToolError::Execution(format!("创建 HTTP 客户端失败: {e}")))?;

        // 城市优先级：用户当轮明说的 > 设置里手动配置的 > IP 自动定位（需开关）
        let (city, lat, lon) = if let Some(city) = stated_city {
            geocode_city(&client, &city).await?
        } else if let Some(city) = configured_city {
            geocode_city(&client, &city).await?
        } else if ip_enabled {
            locate_by_ip(&client).await?
        } else {
            return Err(ToolError::Execution(
                "没有可用的城市信息：用户没有说过所在城市，设置里也未配置城市，\
                 IP 自动定位未开启。本次无法查询天气，请自然地回应，不要追问城市"
                    .into(),
            ));
        };

        let weather = fetch_current(&client, &lat, &lon).await?;
        let current = &weather["current"];
        let code = current["weather_code"].as_i64().unwrap_or(-1);

        // 成功的查询同时喂给上下文缓存：模型之后没调工具也"知道"天气
        store_summary(format_summary(&city, current));

        Ok(json!({
            "city": city,
            "temperature_c": current["temperature_2m"],
            "apparent_temperature_c": current["apparent_temperature"],
            "humidity_percent": current["relative_humidity_2m"],
            "wind_kmh": current["wind_speed_10m"],
            "condition": describe_weather_code(code),
            "effect_hint": effect_hint_for(code),
        }))
    }
}
