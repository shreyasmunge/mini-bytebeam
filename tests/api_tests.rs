//! Integration tests — talk to a real Postgres + real HTTP server.
//!
//! Run:
//!   docker compose up -d db
//!   DATABASE_URL=postgres://fleetpulse:fleetpulse@localhost:5433/fleetpulse cargo test

use chrono::Utc;
use mini_bytebeam::config::Config;
use mini_bytebeam::{app_router, build_state};
use serde_json::{json, Value};
use std::net::SocketAddr;
use uuid::Uuid;

async fn start_test_server() -> String {
    let mut config = Config::from_env();
    // Use whatever DATABASE_URL the shell provides
    config.host = "127.0.0.1".into();
    config.port = 0; // unused; we bind to random port below

    let state = build_state(config)
        .await
        .expect("DB not ready — start Postgres: docker compose up -d db");

    let app = app_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind failed");
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    format!("http://{}", addr)
}

async fn register(base: &str, name: &str) -> (Uuid, String) {
    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/devices", base))
        .json(&json!({ "name": name }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 201);
    let body: Value = res.json().await.unwrap();
    let id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();
    let api_key = body["api_key"].as_str().unwrap().to_string();
    (id, api_key)
}

#[tokio::test]
async fn health_ok() {
    let base = start_test_server().await;
    let res = reqwest::get(format!("{}/health", base)).await.unwrap();
    assert_eq!(res.status(), 200);
}

#[tokio::test]
async fn ready_ok() {
    let base = start_test_server().await;
    let res = reqwest::get(format!("{}/ready", base)).await.unwrap();
    assert_eq!(res.status(), 200);
}

#[tokio::test]
async fn reject_missing_auth() {
    let base = start_test_server().await;
    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/telemetry", base))
        .json(&json!({
            "id": Uuid::new_v4(),
            "recorded_at": Utc::now(),
            "latitude": 1.0,
            "longitude": 2.0,
            "speed_kmh": 10.0,
            "battery_pct": 50.0,
            "engine_temp_c": 90.0
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn reject_bad_battery() {
    let base = start_test_server().await;
    let (_id, key) = register(&base, "bad-battery-truck").await;
    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/telemetry", base))
        .header("Authorization", format!("Bearer {}", key))
        .json(&json!({
            "id": Uuid::new_v4(),
            "recorded_at": Utc::now(),
            "latitude": 1.0,
            "longitude": 2.0,
            "speed_kmh": 10.0,
            "battery_pct": 150.0,
            "engine_temp_c": 90.0
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn telemetry_create_and_duplicate_ignored() {
    let base = start_test_server().await;
    let (device_id, key) = register(&base, "idempotent-truck").await;
    let event_id = Uuid::new_v4();
    let client = reqwest::Client::new();

    let payload = json!({
        "id": event_id,
        "recorded_at": Utc::now(),
        "latitude": 12.97,
        "longitude": 77.59,
        "speed_kmh": 40.0,
        "battery_pct": 70.0,
        "engine_temp_c": 95.0
    });

    let first = client
        .post(format!("{}/telemetry", base))
        .header("Authorization", format!("Bearer {}", key))
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), 201);
    let first_body: Value = first.json().await.unwrap();
    assert_eq!(first_body["status"], "created");

    let second = client
        .post(format!("{}/telemetry", base))
        .header("Authorization", format!("Bearer {}", key))
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(second.status(), 200);
    let second_body: Value = second.json().await.unwrap();
    assert_eq!(second_body["status"], "duplicate_ignored");

    // Device should be readable and online
    let got = client
        .get(format!("{}/devices/{}", base, device_id))
        .header("Authorization", format!("Bearer {}", key))
        .send()
        .await
        .unwrap();
    assert_eq!(got.status(), 200);
    let device: Value = got.json().await.unwrap();
    assert_eq!(device["status"], "online");
}

#[tokio::test]
async fn high_temp_creates_alert() {
    let base = start_test_server().await;
    let (device_id, key) = register(&base, "hot-truck").await;
    let client = reqwest::Client::new();

    let res = client
        .post(format!("{}/telemetry", base))
        .header("Authorization", format!("Bearer {}", key))
        .json(&json!({
            "id": Uuid::new_v4(),
            "recorded_at": Utc::now(),
            "latitude": 12.0,
            "longitude": 77.0,
            "speed_kmh": 10.0,
            "battery_pct": 50.0,
            "engine_temp_c": 120.0
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 201);
    let body: Value = res.json().await.unwrap();
    assert!(body["alerts_created"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "high_engine_temp"));

    let alerts = client
        .get(format!("{}/devices/{}/alerts", base, device_id))
        .header("Authorization", format!("Bearer {}", key))
        .send()
        .await
        .unwrap();
    assert_eq!(alerts.status(), 200);
    let list: Value = alerts.json().await.unwrap();
    assert!(!list.as_array().unwrap().is_empty());
}

#[allow(dead_code)]
fn _silence_unused_socketaddr(_: SocketAddr) {}
