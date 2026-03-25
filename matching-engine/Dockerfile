FROM rust:1.86-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p matching-engine

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
RUN mkdir -p /data
COPY --from=builder /app/target/release/matching-engine /usr/local/bin/matching-engine
COPY --from=builder /app/matching-engine/migrations /app/migrations
CMD ["/usr/local/bin/matching-engine"]
