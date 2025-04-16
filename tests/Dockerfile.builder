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

# Copy Cargo definition files first (better Docker caching)
COPY Cargo.toml Cargo.lock ./

# Create a dummy main file to pre-build dependencies
RUN mkdir -p src && \
    echo 'fn main() { println!("Dummy!"); }' > src/main.rs

# Use buildx cache for dependencies
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo fetch

# Copy actual source code
RUN rm -rf src
COPY src ./src
COPY resources ./resources

# Build the application with cached dependencies
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release --all-features && \
    cp target/release/dela /app/ 