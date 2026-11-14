//! Realistic demo fleet for the map UI.
//!
//! Most vehicles stay healthy + online (green).
//! About 1–2 at a time can be offline or in red zone, then recover —
//! like a real ops console, not random mass failure.
//!
//! Usage: cargo run --bin demo_fleet

use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Role {
    Healthy,
    /// Cycles: normal → brief overheat (red) → cool down
    HeatCycle,
    /// Cycles: online → silent (offline) → back online
    DropCycle,
}

struct Script {
    name: &'static str,
    lat: f64,
    lon: f64,
    role: Role,
}

const FLEET: &[Script] = &[
    Script {
        name: "demo-airport-shuttle",
        lat: 13.1989,
        lon: 77.7068,
        role: Role::Healthy,
    },
    Script {
        name: "demo-whitefield-van",
        lat: 12.9698,
        lon: 77.7500,
        role: Role::Healthy,
    },
    Script {
        name: "demo-ecity-truck",
        lat: 12.8452,
        lon: 77.6602,
        role: Role::HeatCycle,
    },
    Script {
        name: "demo-koramangala-cab",
        lat: 12.9352,
        lon: 77.6245,
        role: Role::Healthy,
    },
    Script {
        name: "demo-yelahanka-bus",
        lat: 13.1007,
        lon: 77.5963,
        role: Role::Healthy,
    },
    Script {
        name: "demo-mysore-road-lorry",
        lat: 12.9510,
        lon: 77.5200,
        role: Role::DropCycle,
    },
    Script {
        name: "demo-indiranagar-scooter",
        lat: 12.9784,
        lon: 77.6408,
        role: Role::Healthy,
    },
    Script {
        name: "demo-hebbal-ev",
        lat: 13.0358,
        lon: 77.5970,
        role: Role::Healthy,
    },
    Script {
        name: "demo-jayanagar-auto",
        lat: 12.9308,
        lon: 77.5838,
        role: Role::Healthy,
    },
    Script {
        name: "demo-marathahalli-van",
        lat: 12.9591,
        lon: 77.6974,
        role: Role::Healthy,
    },
];

#[derive(Debug, Deserialize)]
struct RegisterResponse {
    #[allow(dead_code)]
    id: Uuid,
    api_key: String,
}

struct LiveDevice {
    name: String,
    api_key: String,
    lat: f64,
    lon: f64,
    role: Role,
    tick: u64,
    heat_until: Option<Instant>,
    silent_until: Option<Instant>,
    next_phase_at: Instant,
}

#[tokio::main]
async fn main() {
    let base = std::env::var("API_BASE").unwrap_or_else(|_| "http://127.0.0.1:3000".into());
    let tick_ms: u64 = env_u64("DEMO_TICK_MS", 2500);
    let offline_after: i64 = 20; // so DropCycle shows offline clearly

    let client = reqwest::Client::new();
    println!("demo_fleet → {base}");

    let _ = client
        .put(format!("{base}/settings/thresholds"))
        .json(&json!({
            "engine_temp_alert_c": 110.0,
            "battery_low_pct": 15.0,
            "offline_after_secs": offline_after
        }))
        .send()
        .await;

    match client.delete(format!("{base}/demo/reset")).send().await {
        Ok(res) => println!("demo reset: {}", res.status()),
        Err(err) => eprintln!("demo reset skipped: {err}"),
    }

    let mut fleet = Vec::new();
    let now = Instant::now();
    for (i, script) in FLEET.iter().enumerate() {
        let res = client
            .post(format!("{base}/devices"))
            .json(&json!({ "name": script.name }))
            .send()
            .await
            .expect("register failed");
        if !res.status().is_success() {
            panic!(
                "register {} failed: {}",
                script.name,
                res.text().await.unwrap_or_default()
            );
        }
        let body: RegisterResponse = res.json().await.expect("register json");
        println!("registered {} ({:?})", script.name, script.role);

        // Problem vehicles start their first issue soon; healthy ones never do.
        let first = match script.role {
            Role::Healthy => now + Duration::from_secs(3600),
            Role::HeatCycle => now + Duration::from_secs(25 + i as u64),
            Role::DropCycle => now + Duration::from_secs(35 + i as u64),
        };

        fleet.push(LiveDevice {
            name: script.name.to_string(),
            api_key: body.api_key,
            lat: script.lat,
            lon: script.lon,
            role: script.role,
            tick: 0,
            heat_until: None,
            silent_until: None,
            next_phase_at: first,
        });
    }

    println!("8 healthy green · 1 heat-cycle · 1 drop-cycle (offline)");
    println!("watch http://127.0.0.1:5173/");

    loop {
        let now = Instant::now();
        for d in fleet.iter_mut() {
            d.tick += 1;

            // --- Heat cycle: ~35s red, then ~70s green, repeat ---
            if d.role == Role::HeatCycle {
                if d.heat_until.is_none() && now >= d.next_phase_at {
                    d.heat_until = Some(now + Duration::from_secs(35));
                    d.next_phase_at = now + Duration::from_secs(105);
                    println!("{} → RED (high temp)", d.name);
                }
                if let Some(until) = d.heat_until {
                    if now >= until {
                        d.heat_until = None;
                        println!("{} → recovered (temp normal)", d.name);
                    }
                }
            }

            // --- Drop cycle: silent long enough to go offline, then resume ---
            if d.role == Role::DropCycle {
                if d.silent_until.is_none() && now >= d.next_phase_at {
                    let silence = (offline_after as u64) + 25; // ~45s offline on map
                    d.silent_until = Some(now + Duration::from_secs(silence));
                    d.next_phase_at = now + Duration::from_secs(silence + 80);
                    println!("{} → going OFFLINE (no signal)", d.name);
                }
                if let Some(until) = d.silent_until {
                    if now < until {
                        continue;
                    }
                    d.silent_until = None;
                    println!("{} → back ONLINE", d.name);
                }
            }

            let drift = (d.tick as f64) * 0.00018;
            let lat = d.lat + drift.sin() * 0.005;
            let lon = d.lon + drift.cos() * 0.005;

            let speed = 22.0 + ((d.tick + d.name.len() as u64) % 28) as f64;
            let mut battery = 62.0 + ((d.tick % 20) as f64) * 0.8;
            let mut temp = 84.0 + ((d.tick % 12) as f64) * 0.7;
            if battery > 90.0 {
                battery = 68.0;
            }

            if d.heat_until.is_some() {
                temp = 116.0 + ((d.tick % 3) as f64);
            }

            let payload = json!({
                "id": Uuid::new_v4(),
                "recorded_at": Utc::now(),
                "latitude": lat,
                "longitude": lon,
                "speed_kmh": speed,
                "battery_pct": battery,
                "engine_temp_c": temp,
            });

            if let Err(err) = client
                .post(format!("{base}/telemetry"))
                .header("Authorization", format!("Bearer {}", d.api_key))
                .json(&payload)
                .send()
                .await
            {
                eprintln!("{} error: {err}", d.name);
            }
        }

        tokio::time::sleep(Duration::from_millis(tick_ms)).await;
    }
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}
