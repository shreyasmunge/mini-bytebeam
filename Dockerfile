# Stage 1: build the Rust binary
FROM rust:1.85-bookworm AS builder
WORKDIR /app
COPY Cargo.toml ./
COPY migrations ./migrations
COPY src ./src
# Generate Cargo.lock inside the image if missing
RUN cargo build --release --bin mini-bytebeam

# Stage 2: tiny runtime image
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/mini-bytebeam /usr/local/bin/mini-bytebeam
ENV API_HOST=0.0.0.0
ENV API_PORT=3000
EXPOSE 3000
CMD ["mini-bytebeam"]
