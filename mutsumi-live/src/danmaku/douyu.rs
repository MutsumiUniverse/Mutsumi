use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use async_tungstenite::tungstenite::Message;
use flume::Sender;
use futures::{SinkExt, StreamExt};

use super::LiveDanmaku;

const HEARTBEAT: &[u8] =
    b"\x14\x00\x00\x00\x14\x00\x00\x00\xb1\x02\x00\x00\x74\x79\x70\x65\x40\x3d\x6d\x72\x6b\x6c\x2f\x00";

static COLORS: &[(&str, u32)] = &[
    ("1", 0xff0000),
    ("2", 0x1e87f0),
    ("3", 0x7ac84b),
    ("4", 0xff7f00),
    ("5", 0x9b39f4),
    ("6", 0xff69b4),
];

fn lookup_color(col: &str) -> u32 {
    COLORS
        .iter()
        .find(|(k, _)| *k == col)
        .map(|(_, v)| *v)
        .unwrap_or(0xffffff)
}

pub fn parse_douyu_room_id(url: &str) -> Option<String> {
    let url = url.trim();
    let path = url
        .strip_prefix("https://www.douyu.com/")
        .or_else(|| url.strip_prefix("http://www.douyu.com/"))
        .or_else(|| url.strip_prefix("https://douyu.com/"))
        .or_else(|| url.strip_prefix("http://douyu.com/"))?;
    let rid = path.split(['?', '#', '/']).next()?;
    if rid.is_empty() { None } else { Some(rid.to_string()) }
}

pub async fn check_douyu_live_status(rid: &str) -> Option<bool> {
    use gtk::gio;
    use gtk::prelude::FileExtManual;

    let uri = format!("https://www.douyu.com/betard/{rid}");
    let (contents, _) = gio::File::for_uri(&uri).load_contents_future().await.ok()?;
    let resp: serde_json::Value = serde_json::from_slice(&contents).ok()?;
    let show_status = resp["room"]["show_status"].as_i64()?;
    let video_loop = resp["room"]["videoLoop"].as_i64()?;
    Some(show_status == 1 && video_loop == 0)
}

/// Resolves the RTMP stream URL for a Douyu room.
///
/// Requires `crypto-js.min.js` to be placed alongside this source file.
/// The JS is bundled at compile time via `include_str!`.
pub async fn get_douyu_stream_url(
    rid: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    const API2: &str = "https://www.douyu.com/swf_api/homeH5Enc?rids=";
    const API3: &str = "https://www.douyu.com/lapi/live/getH5Play/";
    const UA: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:120.0) Gecko/20100101 Firefox/120.0";

    let did = uuid::Uuid::new_v4().simple().to_string();

    // acf_did must match the did passed to the signing function
    let cookie = format!("acf_did={did}");
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{API2}{rid}"))
        .header("User-Agent", UA)
        .header("Referer", format!("https://www.douyu.com/{rid}"))
        .header("Cookie", &cookie)
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    let js_enc = resp
        .pointer(&format!("/data/room{rid}"))
        .and_then(|x| x.as_str())
        .ok_or("missing room JS in API2 response")?
        .to_string();

    let crypto_js = include_str!("crypto-js.min.js");
    let tsec = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string();

    let rt = rquickjs::Runtime::new()?;
    let ctx = rquickjs::Context::full(&rt)?;
    let enc_data = ctx.with(|ctx| -> rquickjs::Result<String> {
        ctx.eval::<(), _>(crypto_js)?;
        ctx.eval::<(), _>(js_enc.as_str())?;
        ctx.eval::<String, _>(format!("ub98484234('{rid}','{did}','{tsec}')"))
    })?;

    let mut params: Vec<(String, String)> = enc_data
        .split('&')
        .filter_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            Some((k.to_string(), v.to_string()))
        })
        .collect();
    params.push(("cdn".to_string(), String::new()));
    params.push(("iar".to_string(), "0".to_string()));
    params.push(("ive".to_string(), "0".to_string()));
    params.push(("rate".to_string(), "0".to_string()));

    let resp = client
        .post(format!("{API3}{rid}"))
        .header("User-Agent", UA)
        .header("Referer", format!("https://www.douyu.com/{rid}"))
        .header("Cookie", &cookie)
        .form(&params)
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    let error_code = resp["error"].as_i64().unwrap_or(0);
    if error_code != 0 {
        let msg = resp["msg"].as_str().unwrap_or("unknown error");
        return Err(format!("douyu API error {error_code}: {msg}").into());
    }

    let rtmp_url = resp
        .pointer("/data/rtmp_url")
        .and_then(|x| x.as_str())
        .ok_or("missing rtmp_url in getH5Play response")?;
    let rtmp_live = resp
        .pointer("/data/rtmp_live")
        .and_then(|x| x.as_str())
        .ok_or("missing rtmp_live in getH5Play response")?;

    Ok(format!("{rtmp_url}/{rtmp_live}"))
}

fn build_packet(payload: &str) -> Vec<u8> {
    let len = payload.len() as u32 + 9;
    let mut data = Vec::with_capacity(payload.len() + 13);
    data.extend_from_slice(&len.to_le_bytes());
    data.extend_from_slice(&len.to_le_bytes());
    data.extend_from_slice(b"\xb1\x02\x00\x00");
    data.extend_from_slice(payload.as_bytes());
    data.push(0x00);
    data
}

// Douyu binary frame: [4B len_le][len bytes: [4B len_le][4B magic][payload][1B null][1B sep]]
// payload = msg_len - 10 bytes (excludes the trailing sep and null counted in msg_len)
fn decode_packets(data: &[u8]) -> Vec<LiveDanmaku> {
    let mut ret = Vec::new();
    let mut pos = 0;

    loop {
        if pos + 4 > data.len() {
            break;
        }
        let msg_len = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;
        if msg_len < 10 || pos + msg_len > data.len() {
            break;
        }

        let payload_start = pos + 8;
        let payload_len = msg_len - 10;
        let payload = &data[payload_start..payload_start + payload_len];
        pos += msg_len;

        let msg = String::from_utf8_lossy(payload);
        let msg = msg.replace("@=", r#"":""#).replace('/', r#"",""#);
        let msg = msg.replace("@A", "@").replace("@S", "/");
        let msg = format!(r#"{{"{}"}}"#, &msg);

        let j: serde_json::Value = match serde_json::from_str(&msg) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if j["type"].as_str() != Some("chatmsg") {
            continue;
        }
        let text = match j["txt"].as_str() {
            Some(t) => t.trim().to_string(),
            None => continue,
        };
        if text.is_empty() {
            continue;
        }
        let col = j["col"].as_str().unwrap_or("-1");
        ret.push(LiveDanmaku { text, color: lookup_color(col) });
    }

    ret
}

async fn connect_once(
    rid: &str,
    sender: &Sender<LiveDanmaku>,
    stop: &AtomicBool,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let (ws, _) =
        async_tungstenite::tokio::connect_async("wss://danmuproxy.douyu.com:8505").await?;
    let (mut ws_write, mut ws_read) = ws.split();

    ws_write
        .send(Message::Binary(build_packet(&format!("type@=loginreq/roomid@={rid}/"))))
        .await?;
    ws_write
        .send(Message::Binary(build_packet(&format!("type@=joingroup/rid@={rid}/gid@=1/"))))
        .await?;

    let mut heartbeat = tokio::time::interval(Duration::from_secs(20));
    heartbeat.tick().await; // skip immediate first tick

    while stop.load(Ordering::Relaxed) {
        tokio::select! {
            _ = heartbeat.tick() => {
                ws_write.send(Message::Binary(HEARTBEAT.to_vec())).await?;
            }
            msg = ws_read.next() => {
                let Some(msg) = msg else { return Ok(false) };
                for dm in decode_packets(&msg?.into_data()) {
                    if sender.send(dm).is_err() {
                        return Ok(true);
                    }
                }
            }
        }
    }

    Ok(true)
}

pub fn spawn_douyu_live_danmaku(
    rid: String,
    sender: Sender<LiveDanmaku>,
    stop: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async move {
                let mut backoff = Duration::from_millis(500);
                while stop.load(Ordering::Relaxed) {
                    match connect_once(&rid, &sender, &stop).await {
                        Ok(true) => break,
                        Ok(false) => {}
                        Err(e) => {
                            tracing::warn!("douyu danmaku error for room {rid}: {e}");
                        }
                    }
                    if !stop.load(Ordering::Relaxed) {
                        break;
                    }
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(Duration::from_secs(30));
                }
                tracing::info!("douyu danmaku stopped for room {rid}");
            });
    });
}
