//! High-rate fleet load generator (thousands of vehicles).
//!
//! Examples:
//!   # 1000 vehicles @ 1 Hz via batch of 1 (HTTP)
//!   SIM_DEVICES=1000 SIM_INTERVAL_MS=1000 cargo run --release --bin load_fleet
//!
//!   # denser: each vehicle posts a batch of 10 every second
//!   SIM_DEVICES=500 BATCH_SIZE=10 SIM_INTERVAL_MS=1000 cargo run --release --bin load_fleet
//!
//! Env: API_BASE, SIM_DEVICES, SIM_INTERVAL_MS, BATCH_SIZE, SIM_DURATION_SECS

use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct RegisterResponse {
    api_key: String,
}

#[tokio::main]
async fn main() {
    let base = std::env::var("API_BASE").unwrap_or_else(|_| "http://127.0.0.1:3000".into());
    let devices: usize = std::env::var("SIM_DEVICES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1000);
    let interval_ms: u64 = std::env::var("SIM_INTERVAL_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1000);
    let batch_size: usize = std::env::var("BATCH_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1)
        .max(1);
    let duration_secs: u64 = std::env::var("SIM_DURATION_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);

    let client = reqwest::Client::builder()
        .pool_max_idle_per_host(devices.min(500))
        .build()
        .expect("http client");

    println!(
        "load_fleet → {base} | devices={devices} interval_ms={interval_ms} batch={batch_size} duration={duration_secs}s"
    );
    println!(
        "target ≈ {:.0} msg/s",
        devices as f64 * batch_size as f64 * (1000.0 / interval_ms as f64)
    );

    let mut keys = Vec::with_capacity(devices);
    for i in 0..devices {
        let name = format!("load-v-{}", i + 1);
        let res = client
            .post(format!("{base}/devices"))
            .json(&json!({ "name": name }))
            .send()
            .await
            .expect("register");
        if !res.status().is_success() {
            panic!("register failed: {}", res.text().await.unwrap_or_default());
        }
        let body: RegisterResponse = res.json().await.expect("register json");
        keys.push(body.api_key);
        if (i + 1) % 200 == 0 {
            println!("registered {}/{}", i + 1, devices);
        }
    }

    let ok = Arc::new(AtomicU64::new(0));
    let err = Arc::new(AtomicU64::new(0));
    let events = Arc::new(AtomicU64::new(0));
    let start = Instant::now();
    let deadline = start + Duration::from_secs(duration_secs);

    let mut handles = Vec::new();
    for (i, api_key) in keys.into_iter().enumerate() {
        let client = client.clone();
        let base = base.clone();
        let ok = ok.clone();
        let err = err.clone();
        let events = events.clone();
        handles.push(tokio::spawn(async move {
            let mut tick = 0u64;
            while Instant::now() < deadline {
                tick += 1;
                let mut batch = Vec::with_capacity(batch_size);
                for _ in 0..batch_size {
                    batch.push(json!({
                        "id": Uuid::new_v4(),
                        "recorded_at": Utc::now(),
                        "latitude": 12.90 + (i as f64) * 0.0001,
                        "longitude": 77.50 + ((i % 100) as f64) * 0.001,
                        "speed_kmh": 30.0 + (tick % 40) as f64,
                        "battery_pct": 40.0 + ((tick % 50) as f64),
                        "engine_temp_c": 85.0 + ((tick % 20) as f64),
                    }));
                }

                let res = if batch_size == 1 {
                    client
                        .post(format!("{base}/telemetry"))
                        .header("Authorization", format!("Bearer {api_key}"))
                        .json(&batch[0])
                        .send()
                        .await
                } else {
                    client
                        .post(format!("{base}/telemetry/batch"))
                        .header("Authorization", format!("Bearer {api_key}"))
                        .json(&json!({ "events": batch }))
                        .send()
                        .await
                };

                match res {
                    Ok(r) if r.status().is_success() => {
                        ok.fetch_add(1, Ordering::Relaxed);
                        events.fetch_add(batch_size as u64, Ordering::Relaxed);
                    }
                    _ => {
                        err.fetch_add(1, Ordering::Relaxed);
                    }
                }

                tokio::time::sleep(Duration::from_millis(interval_ms)).await;
            }
        }));
    }

    // Progress reporter
    let ok_r = ok.clone();
    let err_r = err.clone();
    let events_r = events.clone();
    let reporter = tokio::spawn(async move {
        let mut last_events = 0u64;
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let e = events_r.load(Ordering::Relaxed);
            let delta = e.saturating_sub(last_events);
            last_events = e;
            println!(
                "progress ok_req={} err={} events={} (~{} evt/s last 5s)",
                ok_r.load(Ordering::Relaxed),
                err_r.load(Ordering::Relaxed),
                e,
                delta / 5
            );
            if Instant::now() >= deadline + Duration::from_secs(2) {
                break;
            }
        }
    });

    for h in handles {
        let _ = h.await;
    }
    let _ = reporter.await;

    let elapsed = start.elapsed().as_secs_f64().max(0.001);
    let total_events = events.load(Ordering::Relaxed);
    println!(
        "DONE elapsed={elapsed:.1}s events={total_events} ok_req={} err={} avg={:.0} msg/s",
        ok.load(Ordering::Relaxed),
        err.load(Ordering::Relaxed),
        total_events as f64 / elapsed
    );
}
