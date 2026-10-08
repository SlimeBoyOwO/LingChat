use async_trait::async_trait;
use once_cell::sync::Lazy;
use serde_json::{Value, json};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri_plugin_store::StoreExt;

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

/// 今日首次对话时返回应注入的天气提醒（`None` = 本次不注入）。
///
/// 注入口径（review 定稿）：天气不逐轮进入上下文——模型平时靠 `query_weather`
/// 工具自查，只有"今日第一句"追加一条带日期的天气提醒。多重门依次为：
/// 调用方环境允许（剧本/试玩由 generator 侧排除）、缓存有货（后台刷新循环
/// 维护）、配置开关打开、今天还没注入过。缓存无货时不记账——等到后台刷新
/// 上来后，当天的后续对话仍能补上这条提醒。
pub(crate) fn first_talk_reminder(app: &tauri::AppHandle, allowed: bool) -> Option<String> {
    if !allowed {
        return None;
    }
    let summary = cached_summary()?;
    if !AppConfig::load(app).ok()?.weather_first_talk {
        return None;
    }

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let store = app.store(crate::config::STORE_FILE).ok()?;
    let already = store
        .get(crate::config::session::LAST_WEATHER_TALK_DATE)
        .and_then(|v| v.as_str().map(String::from));
    if already.as_deref() == Some(today.as_str()) {
        return None;
    }
    store.set(
        crate::config::session::LAST_WEATHER_TALK_DATE.to_string(),
        Value::String(today),
    );
    let _ = store.save();

    Some(format!("用户所在地天气：{summary}"))
}

/// 后台刷新循环：每 5 分钟检查一次，缓存缺失/过期且配置了手动城市时重新拉取。
/// 没配城市就不刷新——天气感知只在用户主动指定过城市时工作，不做自动定位。
/// 由 lib.rs 在启动时 spawn。
pub async fn run_refresh_loop(app: tauri::AppHandle) {
    loop {
        let city = AppConfig::load(&app).ok().and_then(|c| configured_city(&c));

        let stale = WEATHER_CACHE
            .lock()
            .ok()
            .and_then(|c| c.as_ref().map(|w| w.fetched_at.elapsed() > WEATHER_TTL))
            .unwrap_or(true);

        if let (true, Some(city)) = (stale, city) {
            match reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
            {
                Ok(client) => match fetch_summary_for_city(&client, &city).await {
                    Ok(summary) => store_summary(summary),
                    Err(e) => tracing::warn!("天气缓存刷新失败: {e}"),
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

/// 城市名 → 自然语言摘要（geocode + 拉取 + 组装）。
async fn fetch_summary_for_city(client: &reqwest::Client, city: &str) -> Result<String, ToolError> {
    let (city, lat, lon) = geocode_city(client, city).await?;
    let weather = fetch_current(client, &lat, &lon).await?;
    Ok(format_summary(&city, &weather["current"]))
}

/// query_weather：查询实时天气（Open-Meteo，免 API key）。
///
/// 城市两级来源：用户在对话中明确说出的城市优先；未说时使用设置里手动配置的
/// 城市；两者皆无则本次不查询，且不追问用户。
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
            "获取用户所在地（设置中配置的城市）的实时天气。\
             返回结果只是给你自己看的参考——**绝不要在回复里罗列温度、湿度等原始数据**，\
             用一两句自然的口语把天气感受融进对话（例如「外面下着雨呢，出门记得带把伞」）。\
             平时不必主动关注天气；用户提到想出去玩、出门、旅行、问穿什么衣服这类话题时才顺手查一下。\
             拿到结果后可以顺手用 set_background_effect 切粒子特效（effect_hint 字段给了建议值）、\
             必要时用 scene_switch 换更贴合的背景、用 change_clothes 换身合适的衣服",
            json!({
                "type": "object",
                "properties": {
                    "city": {"type": "string", "description": "城市中文名；仅当用户在对话中明确说了别的城市时才填"}
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
        let configured = AppConfig::load(&app).ok().and_then(|c| configured_city(&c));

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| ToolError::Execution(format!("创建 HTTP 客户端失败: {e}")))?;

        // 城市两级来源：用户当轮明说的 > 设置里手动配置的；都没有就不查、不追问
        let (city, lat, lon) = if let Some(city) = stated_city {
            geocode_city(&client, &city).await?
        } else if let Some(city) = configured {
            geocode_city(&client, &city).await?
        } else {
            return Err(ToolError::Execution(
                "没有可用的城市信息：用户没有说过所在城市，设置里也未配置城市。\
                 本次无法查询天气，请自然地回应，不要追问城市"
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
