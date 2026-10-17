FROM rust:1.98.0 AS rust-builder
WORKDIR /app
# The lock file pins the dependency versions tested locally
COPY Cargo.toml Cargo.lock /app/
COPY src /app/src/
RUN cargo build --release --locked

# Vite build of the single-page web app (web/), emitted into web/dist/
FROM node:22-alpine AS web-builder
WORKDIR /app
COPY package.json package-lock.json /app/
COPY packages/shared /app/packages/shared/
COPY web /app/web/
RUN npm ci
RUN npm run build

# The same Debian release as the rust image, so the binary's glibc matches
FROM debian:trixie-slim AS runtime
WORKDIR /app
COPY --from=rust-builder /app/target/release/pfnext /app/pfnext
COPY --from=web-builder /app/web/dist /app/web/dist
COPY schema.sql /app/schema.sql
COPY Config.toml /app/Config.toml
COPY user_agents.yaml /app/user_agents.yaml
# ca-certificates: the S3 client trusts the system's root certificates
RUN apt-get update \
    && apt-get -y install --no-install-recommends libssl3t64 ca-certificates \
    && rm -rf /var/lib/apt/lists/*
EXPOSE 8000
ENTRYPOINT ["/app/pfnext"]
