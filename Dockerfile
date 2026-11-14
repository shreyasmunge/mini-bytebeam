# Stage 1: build the Rust binaries
FROM rust:1.91-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
COPY src ./src
# API only on Render (mqtt_bridge is optional / local Docker Compose).
RUN cargo build --release --bin mini-bytebeam

# Stage 2: tiny runtime image
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/mini-bytebeam /usr/local/bin/mini-bytebeam
ENV API_HOST=0.0.0.0
ENV PORT=3000
ENV API_PORT=3000
ENV DB_MAX_CONNECTIONS=10
ENV PLAYGROUND_MAX_EVENTS=200
EXPOSE 3000
CMD ["mini-bytebeam"]
