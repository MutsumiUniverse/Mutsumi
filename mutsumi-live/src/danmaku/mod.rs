use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use bililive::{ConfigBuilder, RetryConfig};
use bililive::{Operation, connect::tokio::connect_with_retry};
use flume::Sender;
use futures::StreamExt;
use serde_json::Value;

pub struct LiveDanmaku {
    pub text: String,
    pub color: u32,
}

pub fn parse_bilibili_live_room_id(url: &str) -> Option<u64> {
    let path = url
        .trim()
        .strip_prefix("https://live.bilibili.com/")
        .or_else(|| url.trim().strip_prefix("http://live.bilibili.com/"))?;
    path.split(['?', '#', '/']).next()?.parse::<u64>().ok()
}

async fn resolve_real_room_id(
    client: &reqwest::Client,
    room_id: u64,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    let resp: Value = client
        .get(format!(
            "https://api.live.bilibili.com/room/v1/Room/room_init?id={room_id}"
        ))
        .send()
        .await?
        .json()
        .await?;
    resp["data"]["room_id"]
        .as_u64()
        .ok_or_else(|| format!("missing room_id in room_init response: {resp}").into())
}

async fn fetch_room_danmu_info(
    room_id: u64,
) -> Result<(u64, String, Vec<String>), Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();

    let real_room_id = resolve_real_room_id(&client, room_id).await?;

    let resp: Value = client
        .get(format!(
            "https://api.live.bilibili.com/room/v1/Danmu/getConf?room_id={real_room_id}"
        ))
        .send()
        .await?
        .json()
        .await?;

    let token = resp["data"]["token"]
        .as_str()
        .ok_or_else(|| format!("missing token in getConf response: {resp}"))?
        .to_string();
    let servers = resp["data"]["host_server_list"]
        .as_array()
        .ok_or_else(|| format!("missing host_server_list in getConf response: {resp}"))?
        .iter()
        .filter_map(|h| {
            let host = h["host"].as_str()?;
            let wss_port = h["wss_port"].as_u64()?;
            Some(format!("wss://{host}:{wss_port}/sub"))
        })
        .collect();

    Ok((real_room_id, token, servers))
}

pub fn spawn_bilibili_live_danmaku(
    room_id: u64,
    sender: Sender<LiveDanmaku>,
    stop: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async move {
                let (real_room_id, token, servers) = match fetch_room_danmu_info(room_id).await {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::error!(
                            "bilibili live config fetch failed for room {room_id}: {e}"
                        );
                        return;
                    }
                };

                let config = ConfigBuilder::new()
                    .room_id(real_room_id)
                    .uid(0)
                    .token(&token)
                    .servers(&servers)
                    .build();

                let mut stream = match connect_with_retry(config, RetryConfig::default()).await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("bilibili live connect failed for room {room_id}: {e}");
                        return;
                    }
                };

                while stop.load(Ordering::Relaxed) {
                    let Some(result) = stream.next().await else {
                        break;
                    };
                    match result {
                        Ok(packet) if packet.op() == Operation::Notification => {
                            if let Ok(json) = packet.json::<Value>() {
                                if json["cmd"].as_str() == Some("DANMU_MSG") {
                                    let text = json["info"][1].as_str().unwrap_or("").to_string();
                                    if !text.is_empty() {
                                        let color =
                                            json["info"][0][3].as_u64().unwrap_or(0xFF_FFFF) as u32;
                                        if sender.send(LiveDanmaku { text, color }).is_err() {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("bilibili live stream error for room {room_id}: {e}");
                        }
                        _ => {}
                    }
                }

                tracing::info!("bilibili live danmaku stopped for room {room_id}");
            });
    });
}
