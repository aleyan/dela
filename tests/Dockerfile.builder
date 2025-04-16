# syntax=docker/dockerfile:1.4
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

# Configure Cargo for better caching
ENV CARGO_INCREMENTAL=1
ENV CARGO_TARGET_DIR=/app/target
# Don't use crt-static as it might cause problems with proc macros
ENV RUSTFLAGS=""
ENV RUSTC_WRAPPER=""

# Copy Cargo definition files first (better Docker caching)
COPY Cargo.toml Cargo.lock ./

# Create a dummy main file to pre-build dependencies
RUN mkdir -p src && \
    echo 'fn main() { println!("Dummy!"); }' > src/main.rs

# Use buildx cache for dependencies - critical step for caching
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=cargo-target,target=/app/target \
    cargo fetch && \
    cargo build --release --all-features || true

# Copy actual source code
RUN rm -rf src
COPY src ./src
COPY resources ./resources

# Build the application with cached dependencies and all features
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=cargo-target,target=/app/target \
    cargo build --release --all-features && \
    cp target/release/dela /app/ && \
    strip /app/dela 