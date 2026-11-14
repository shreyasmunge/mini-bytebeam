# What to add to feel more like Bytebeam (product lens)

Bytebeam’s product is a **connected-vehicle / IoT operations platform**: ingest device data, monitor fleets, diagnose issues, manage devices, and (in the real product) push OTA updates.

FleetPulse already shows: telemetry ingest, health, alerts, map, thresholds. Below is what would make a hiring manager think “this person understands our domain.”

## High value (add next if you have time)

| Feature | Why it feels like Bytebeam |
|---------|----------------------------|
| **Device twin / last-known state panel** | Click a vehicle → full diagnostic drawer (ignition-ish fields, battery, GPS, last errors) |
| **Alert inbox + acknowledge** | Ops workflow: triage alerts, mark resolved |
| **Historical trail on map** | Last N GPS points as a polyline (trip reconstruction lite) |
| **Actions / commands** | `POST /devices/:id/actions` e.g. “request reboot” / “honk” stub — even if simulated |
| **OTA update stub** | Firmware version on device + “roll out v1.2” job status (queued → sent → done) |
| **MQTT note in README** | Bytebeam cares about MQTT at scale; say what you’d add (topic design, broker) without building it yet |

## Medium value

| Feature | Why |
|---------|-----|
| Multi-tenant / project id | `project_id` on devices (OEM vs fleet customer) |
| Role-based UI | Viewer vs operator (even fake login) |
| Dashboards KPIs | Active / idle / maintenance / offline counts (Bytebeam-style control room) |
| Geofence alert | Simple polygon or radius around depot |
| Export / CSV | Fleet report for a time range |

## What Bytebeam has that you should **mention**, not necessarily build

- In-vehicle **Rust agent** (like uplink) talking to cloud  
- **Message broker** for millions of devices  
- Real **OTA** pipelines and signed artifacts  
- Deep OEM integrations (CAN, diagnostics)  
- Clojure/React production console polish  

In interviews: “I scoped FleetPulse to the cloud ingestion + ops console slice; the edge agent and broker would be the next layer.”

## Keep the story honest

Your strongest framing:

1. Production backend experience (Tabbly)  
2. Rust API with idempotent telemetry + tests + load numbers  
3. React/TS console that matches their frontend stack  
4. Clear next steps toward Bytebeam’s full product (MQTT, OTA, actions)
