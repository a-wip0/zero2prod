# ==========================================
# 1. BUILD STAGE
# ==========================================
FROM rust:1.97.1-slim-trixie AS builder

WORKDIR /app

# Install build tools and custom linkers needed for compilation
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    clang \
    lld \
    mold \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY . .

ENV SQLX_OFFLINE=true
RUN cargo build --release

# ==========================================
# 2. RUNTIME STAGE
# ==========================================
FROM debian:trixie-slim AS runtime

WORKDIR /app

# Install runtime utilities (SSL for network calls, CA certs for HTTPS)
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends openssl ca-certificates \
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/zero2prod zero2prod
COPY configuration configuration

ENV APP_ENVIRONMENT=production

ENTRYPOINT ["./zero2prod"]
