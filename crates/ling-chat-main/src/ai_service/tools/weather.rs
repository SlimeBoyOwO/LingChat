use async_trait::async_trait;
use serde_json::{Value, json};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri_plugin_store::StoreExt;

use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::types::ToolDefinition;
use crate::config::AppConfig;

use super::executor::{Tool, ToolContext, ToolError, ToolResult};
use super::game_status_handle;

// ============ 天气上下文缓存 ============
//
// 天气缓存：存进存档全局变量（`GameStatus::global_variables`，键
// `weather.cache`），随存档快照持久化，重启不丢。query_weather 成功与
// 后台刷新循环两处填充；今日首聊提醒与天气特效同步从缓存读取。

const WEATHER_TTL: Duration = Duration::from_secs(30 * 60);

/// 天气缓存在存档全局变量里的键（与 affection.* 同一命名空间，随存档隔离）。
const CACHE_VAR_KEY: &str = "weather.cache";

/// 当前 Unix 时间戳（秒）。系统时钟异常时返回 0，只会让缓存被判过期，安全。
fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 从存档全局变量读取未过期的自然语言天气摘要，如「成都 小雨 21.5°C（体感 20°C）」。
fn cached_summary_in(gs: &GameStatus) -> Option<String> {
    let cache = gs.get_variable(CACHE_VAR_KEY)?;
    let summary = cache.get("summary")?.as_str()?.to_string();
    let fetched_at = cache.get("fetched_at")?.as_i64()?;
    (unix_now() - fetched_at <= WEATHER_TTL.as_secs() as i64).then_some(summary)
}

/// 从存档全局变量读取未过期缓存的天气码（自动同步特效用）。
fn cached_code_in(gs: &GameStatus) -> Option<i64> {
    let cache = gs.get_variable(CACHE_VAR_KEY)?;
    let code = cache.get("code")?.as_i64()?;
    let fetched_at = cache.get("fetched_at")?.as_i64()?;
    (unix_now() - fetched_at <= WEATHER_TTL.as_secs() as i64).then_some(code)
}

/// 缓存缺失或已过期（后台刷新循环判断是否需要重新拉取）。
fn cache_is_stale(gs: &GameStatus) -> bool {
    let fetched_at = gs
        .get_variable(CACHE_VAR_KEY)
        .and_then(|v| v.get("fetched_at"))
        .and_then(Value::as_i64);
    match fetched_at {
        Some(t) => unix_now() - t > WEATHER_TTL.as_secs() as i64,
        None => true,
    }
}

fn store_summary_in(gs: &mut GameStatus, summary: String, code: i64) {
    gs.set_variable(
        CACHE_VAR_KEY,
        json!({ "summary": summary, "code": code, "fetched_at": unix_now() }),
    );
}

/// 读取配置里的手动城市（trim 后非空才视为有效）。
fn configured_city(config: &AppConfig) -> Option<String> {
    let city = config.weather_city.trim().to_string();
    (!city.is_empty()).then_some(city)
}

/// 今日首次对话时返回应注入的天气提醒（`None` = 本次不注入）。
///
/// 注入口径（review 定稿）：天气不逐轮进入上下文——模型平时靠 `query_weather`
/// 工具自查，只有"今日第一句"由 generator 添加一条带日期的天气台词。多重门
/// 依次为：调用方环境允许（剧本/试玩由 generator 侧排除）、缓存有货（后台
/// 刷新循环维护）、配置开关打开、今天还没注入过。缓存无货时不记账——等到
/// 后台刷新上来后，当天的后续对话仍能补上这条提醒。
///
/// 调用方必须已持有 `game_status` 锁并传入守卫（generator 生成上下文时正好
/// 持锁），本函数内不再重复加锁。
pub(crate) fn first_talk_reminder(
    app: &tauri::AppHandle,
    gs: &GameStatus,
    allowed: bool,
) -> Option<String> {
    if !allowed {
        return None;
    }
    let summary = cached_summary_in(gs)?;
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
/// 由 src-tauri 的 background.rs 在启动时 spawn。
pub async fn run_refresh_loop(app: tauri::AppHandle) {
    loop {
        let city = AppConfig::load(&app).ok().and_then(|c| configured_city(&c));

        // 只在判过期/写缓存时短暂持锁，网络请求期间不持有 game_status 锁
        let stale = {
            let gs = game_status_handle(&app).await;
            let gs = gs.lock().await;
            cache_is_stale(&gs)
        };

        if let (true, Some(city)) = (stale, city) {
            match reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
            {
                Ok(client) => match fetch_summary_for_city(&client, &city).await {
                    Ok((summary, code)) => {
                        {
                            let gs = game_status_handle(&app).await;
                            let mut guard = gs.lock().await;
                            store_summary_in(&mut guard, summary, code);
                        }
                        // 自动同步开着：把现实天气落到天气特效层
                        let sync = AppConfig::load(&app)
                            .map(|c| c.weather_auto_sync)
                            .unwrap_or(false);
                        if sync {
                            sync_weather_effect(&app).await;
                        }
                    },
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

/// WMO 天气码 → 前端**天气层**特效名（particles/index.ts 的 WEATHER_EFFECTS 键值）。
/// 晴/多云返回 "None"（清除天气层）。与 effect_hint_for（氛围层）是两套名字，勿混用。
fn weather_effect_for(code: i64) -> &'static str {
    match code {
        45 | 48 => "Fog",
        51..=57 => "Drizzle",
        61..=67 | 80..=82 => "Rain",
        71..=77 | 85 | 86 => "Snow",
        95..=99 => "Thunderstorm",
        _ => "None",
    }
}

/// 把缓存中的现实天气应用到天气特效层（广播 `ambient:effect`，带 layer=weather）。
/// 缓存无货/过期时跳过，等刷新循环拉到新天气再同步。
pub async fn sync_weather_effect(app: &tauri::AppHandle) {
    let gs = game_status_handle(app).await;
    let Some(code) = ({
        let guard = gs.lock().await;
        cached_code_in(&guard)
    }) else {
        tracing::info!("[WeatherSync] 缓存无货，跳过本次同步");
        return;
    };
    let effect = weather_effect_for(code);
    let payload = json!({
        "type": "ambient_effect",
        "effect": effect,
        "duration": Value::Null,
        "layer": "weather",
    });
    use tauri::Emitter;
    if let Err(e) = app.emit("ambient:effect", &payload) {
        tracing::warn!("[WeatherSync] emit ambient:effect 失败: {e}");
    } else {
        tracing::info!("[WeatherSync] 已同步天气层特效: {effect}（code={code}）");
    }
}

/// 立即拉取一次天气并同步特效，不等下一个刷新周期。
/// 由「打开自动同步」「同步开启时改城市」触发（后台执行）；失败静默，刷新循环兜底。
pub async fn fetch_and_apply_now(app: &tauri::AppHandle) {
    let Some(city) = AppConfig::load(app).ok().and_then(|c| configured_city(&c)) else {
        return;
    };
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("[WeatherSync] 天气客户端创建失败: {e}");
            return;
        },
    };
    match fetch_summary_for_city(&client, &city).await {
        Ok((summary, code)) => {
            {
                let gs = game_status_handle(app).await;
                let mut guard = gs.lock().await;
                store_summary_in(&mut guard, summary, code);
            }
            sync_weather_effect(app).await;
        },
        Err(e) => tracing::warn!("[WeatherSync] 天气立即拉取失败（刷新循环会兜底）: {e}"),
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

/// 城市名 → (自然语言摘要, WMO 天气码)（geocode + 拉取 + 组装）。
async fn fetch_summary_for_city(
    client: &reqwest::Client,
    city: &str,
) -> Result<(String, i64), ToolError> {
    let (city, lat, lon) = geocode_city(client, city).await?;
    let weather = fetch_current(client, &lat, &lon).await?;
    let code = weather["current"]["weather_code"].as_i64().unwrap_or(-1);
    Ok((format_summary(&city, &weather["current"]), code))
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
        let summary = format_summary(&city, current);
        let gs = game_status_handle(&app).await;
        let mut guard = gs.lock().await;
        store_summary_in(&mut guard, summary, code);

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
