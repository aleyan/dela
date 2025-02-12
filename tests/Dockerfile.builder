# --- Stage 1: Builder ---
FROM rust:alpine3.21 AS builder

# Install build dependencies
RUN apk add --no-cache \
    musl-dev \
    gcc \
    make \
    openssl-dev \
    pkgconfig

# Set the working directory inside the container
WORKDIR /app

# Copy Cargo definition files first (better Docker caching)
COPY Cargo.toml Cargo.lock ./

# Pre-fetch dependencies (creates a build cache layer)
RUN cargo fetch || true

# Now copy source code and build
COPY src ./src
COPY resources ./resources
RUN cargo build --release --all-features 