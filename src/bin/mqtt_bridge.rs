//! MQTT → HTTP bridge.
//!
//! Devices publish JSON to: fleet/{device_id}/telemetry
//! Payload must include api_key + telemetry fields (same as POST /telemetry).
//!
//! Env:
//!   MQTT_HOST=127.0.0.1  MQTT_PORT=1883
//!   API_BASE=http://127.0.0.1:3000
//!   MQTT_TOPIC=fleet/+/telemetry
//!
//! Run: cargo run --bin mqtt_bridge

use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct MqttTelemetry {
    api_key: String,
    id: Uuid,
    recorded_at: chrono::DateTime<chrono::Utc>,
    latitude: f64,
    longitude: f64,
    speed_kmh: f64,
    battery_pct: f64,
    engine_temp_c: f64,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let host = std::env::var("MQTT_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = std::env::var("MQTT_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1883);
    let topic = std::env::var("MQTT_TOPIC").unwrap_or_else(|_| "fleet/+/telemetry".into());
    let api_base = std::env::var("API_BASE").unwrap_or_else(|_| "http://127.0.0.1:3000".into());

    let mut mqttoptions = MqttOptions::new("fleetpulse-bridge", host, port);
    mqttoptions.set_keep_alive(Duration::from_secs(30));

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 256);
    client
        .subscribe(&topic, QoS::AtLeastOnce)
        .await
        .expect("subscribe failed");

    tracing::info!("MQTT bridge subscribed to {topic} → {api_base}/telemetry");

    let http = reqwest::Client::new();
    let mut buffer: Vec<(String, serde_json::Value)> = Vec::new();
    let mut last_flush = tokio::time::Instant::now();

    loop {
        tokio::select! {
            event = eventloop.poll() => {
                match event {
                    Ok(Event::Incoming(Incoming::Publish(p))) => {
                        if let Ok(msg) = serde_json::from_slice::<MqttTelemetry>(&p.payload) {
                            let key = msg.api_key.clone();
                            let body = json!({
                                "id": msg.id,
                                "recorded_at": msg.recorded_at,
                                "latitude": msg.latitude,
                                "longitude": msg.longitude,
                                "speed_kmh": msg.speed_kmh,
                                "battery_pct": msg.battery_pct,
                                "engine_temp_c": msg.engine_temp_c,
                            });
                            buffer.push((key, body));
                        } else {
                            tracing::warn!("bad MQTT payload on {}", p.topic);
                        }
                    }
                    Ok(_) => {}
                    Err(err) => {
                        tracing::error!("mqtt error: {err}");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
        }

        let should_flush = buffer.len() >= 200 || last_flush.elapsed() > Duration::from_millis(200);
        if should_flush && !buffer.is_empty() {
            flush_buffer(&http, &api_base, &mut buffer).await;
            last_flush = tokio::time::Instant::now();
        }
    }
}

async fn flush_buffer(
    http: &reqwest::Client,
    api_base: &str,
    buffer: &mut Vec<(String, serde_json::Value)>,
) {
    // Group by api_key so each batch hits one device auth.
    use std::collections::HashMap;
    let mut by_key: HashMap<String, Vec<serde_json::Value>> = HashMap::new();
    for (key, body) in buffer.drain(..) {
        by_key.entry(key).or_default().push(body);
    }

    for (api_key, events) in by_key {
        if events.len() == 1 {
            let _ = http
                .post(format!("{api_base}/telemetry"))
                .header("Authorization", format!("Bearer {api_key}"))
                .json(&events[0])
                .send()
                .await;
        } else {
            let _ = http
                .post(format!("{api_base}/telemetry/batch"))
                .header("Authorization", format!("Bearer {api_key}"))
                .json(&json!({ "events": events }))
                .send()
                .await;
        }
    }
}
