# Multi stage build. The final image carries the binary and nothing else: no
# toolchain, no source, no cargo registry.
#
# Build for the architecture the task definition asks for, which is arm64
# (Graviton), roughly 20 percent cheaper for identical work:
#
#   docker buildx build --platform linux/arm64 -t restaurant-api -f api/Dockerfile .
#
# Run it from the repository root, not from api/, because the build needs the
# committed .sqlx cache alongside the source.

FROM rust:1-slim-trixie AS builder

WORKDIR /build

# The SQLx query! macros check SQL at compile time. In here there is no database
# to check against, so they read the committed offline cache instead. If this
# fails saying a query is not cached, someone changed SQL without running
# `pnpm sqlx:prepare`.
ENV SQLX_OFFLINE=true

# Dependencies first, in their own layer, so editing a source file does not
# rebuild the whole dependency tree.
COPY api/Cargo.toml api/Cargo.lock ./
RUN mkdir -p src/bin \
    && echo 'fn main() {}' > src/main.rs \
    && echo '' > src/lib.rs \
    && echo 'fn main() {}' > src/bin/openapi.rs \
    && cargo build --release \
    && rm -rf src

COPY api/src ./src
COPY api/.sqlx ./.sqlx

# Cargo will not rebuild a crate whose files look untouched, and the dummy
# sources above left stale fingerprints behind.
RUN touch src/main.rs src/lib.rs \
    && cargo build --release --bin api \
    && strip target/release/api


FROM debian:trixie-slim AS runtime

# ca-certificates for outbound TLS, curl for the container health check.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

# Never root.
RUN useradd --system --create-home --uid 10001 api
USER api
WORKDIR /home/api

COPY --from=builder --chown=api:api /build/target/release/api /usr/local/bin/api

EXPOSE 8080

# Config comes from the task definition, which reads it from SSM Parameter
# Store. Nothing sensitive is baked into this image.
ENV APP_HOST=0.0.0.0 \
    APP_PORT=8080 \
    APP_ENV=production

CMD ["api"]
