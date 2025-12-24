FROM rust:1.90 as builder
WORKDIR /usr/src/app
COPY . .
RUN cargo install --path .

FROM debian:sid-slim
RUN apt-get update && apt-get install -y openssl ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/mqtt-to-influx /usr/local/bin/mqtt-to-influx
CMD ["mqtt-to-influx"]