FROM rust:1.83-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.toml
COPY src src
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/kdb-stub /usr/local/bin/
ENTRYPOINT ["kdb-stub"]
