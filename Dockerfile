FROM rust:1.90.0 AS rust-builder
WORKDIR /app
COPY Cargo.toml /app/
COPY src /app/src/
RUN cargo build --release

# Vite build of the single-page web app (web/), emitted into web/dist/
FROM node:22-alpine AS web-builder
WORKDIR /app
COPY package.json package-lock.json /app/
COPY packages/shared /app/packages/shared/
COPY web /app/web/
RUN npm ci
RUN npm run build

FROM debian:bookworm-slim AS runtime
WORKDIR /app
COPY --from=rust-builder /app/target/release/pfnext /app/pfnext
# Tera templates and legacy assets are still needed for the blog pages
COPY templates /app/templates
COPY static /app/static
COPY --from=web-builder /app/web/dist /app/web/dist
COPY schema.sql /app/schema.sql
COPY Config.toml /app/Config.toml
COPY user_agents.yaml /app/user_agents.yaml
RUN apt-get update
RUN apt-get -y install libssl3
EXPOSE 8000
ENTRYPOINT ["/app/pfnext"]
