FROM rust:1.86.0 AS builder
WORKDIR /app
COPY Cargo.toml /app/
COPY src /app/src/
# COPY templates /app/templates/
RUN cargo build --release

FROM debian:bookworm-slim AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/pfnext /app/pfnext

ENTRYPOINT ["/app/pfnext"]