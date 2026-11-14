//! Tiny vehicle fleet simulator.
//!
//! Usage (API must already be running):
//!   cargo run --bin simulator
//!
//! What it does:
//! 1. Registers N devices
//! 2. Each device posts telemetry in a loop
//! This proves the API works under concurrent clients.

use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct RegisterResponse {
    id: Uuid,
    api_key: String,
}

#[tokio::main]
async fn main() {
    let base = std::env::var("API_BASE").unwrap_or_else(|_| "http://127.0.0.1:3000".into());
    let devices: usize = std::env::var("SIM_DEVICES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    let interval_ms: u64 = std::env::var("SIM_INTERVAL_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2000);

    let client = reqwest::Client::new();
    println!("simulator → {} | devices={} interval_ms={}", base, devices, interval_ms);

    let mut keys = Vec::new();
    for i in 0..devices {
        let name = format!("sim-vehicle-{}", i + 1);
        let res = client
            .post(format!("{}/devices", base))
            .json(&json!({ "name": name }))
            .send()
            .await
            .expect("register request failed");
        if !res.status().is_success() {
            panic!("register failed: {}", res.text().await.unwrap_or_default());
        }
        let body: RegisterResponse = res.json().await.expect("bad register json");
        println!("registered {} → {}", name, body.id);
        keys.push(body.api_key);
    }

    // Concurrent loops — one task per device
    let mut handles = Vec::new();
    for (i, api_key) in keys.into_iter().enumerate() {
        let client = client.clone();
        let base = base.clone();
        handles.push(tokio::spawn(async move {
            let mut tick = 0u64;
            loop {
                tick += 1;
                let event_id = Uuid::new_v4();
                let payload = json!({
                    "id": event_id,
                    "recorded_at": Utc::now(),
                    "latitude": 12.97 + (i as f64) * 0.01,
                    "longitude": 77.59,
                    "speed_kmh": 30.0 + (tick % 40) as f64,
                    "battery_pct": 80.0 - ((tick % 50) as f64) * 0.5,
                    "engine_temp_c": 85.0 + ((tick % 20) as f64),
                });

                match client
                    .post(format!("{}/telemetry", base))
                    .header("Authorization", format!("Bearer {}", api_key))
                    .json(&payload)
                    .send()
                    .await
                {
                    Ok(res) => {
                        println!(
                            "device-{} tick={} status={}",
                            i + 1,
                            tick,
                            res.status()
                        );
                    }
                    Err(err) => eprintln!("device-{} error: {}", i + 1, err),
                }

                tokio::time::sleep(Duration::from_millis(interval_ms)).await;
            }
        }));
    }

    for h in handles {
        let _ = h.await;
    }
}
