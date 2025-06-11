FROM jekyll/jekyll AS jekyll-builder
WORKDIR /app
COPY jekyll /app
RUN chmod -R 777 /app
RUN jekyll build --verbose --trace

FROM rust:1.87.0 AS rust-builder
WORKDIR /app
COPY Cargo.toml /app/
COPY src /app/src/
RUN cargo build --release

FROM debian:bookworm-slim AS runtime
WORKDIR /app
COPY --from=jekyll-builder /app/_site /app/static
COPY --from=rust-builder /app/target/release/pfnext /app/pfnext
COPY schema.sql /app/schema.sql
RUN apt-get update
RUN apt-get -y install libssl3
EXPOSE 8000
ENTRYPOINT ["/app/pfnext"]
