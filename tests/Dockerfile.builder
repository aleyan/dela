# syntax=docker/dockerfile:1.4
# --- Stage 1: Builder ---
FROM rust:alpine3.21 AS builder

# Install build dependencies
RUN apk add --no-cache \
    musl-dev \
    gcc \
    make \
    openssl-dev \
    pkgconfig \
    procps

# Set the working directory inside the container
WORKDIR /app

# Copy Cargo definition files first (better Docker caching)
COPY Cargo.toml Cargo.lock ./

# Create dummy source files to compile dependencies
# This builds a skeleton with empty files that will compile the dependencies
RUN mkdir -p src && \
    echo 'fn main() { println!("Dummy!"); }' > src/main.rs && \
    find . -name "*.rs" -not -path "./src/main.rs" -exec touch {} \; && \
    # Build dependencies (debug mode only)
    cargo build --all-features && \
    cargo test --all-features --no-run && \
    rm -rf src

# Now copy the real source code
COPY src/ ./src/
COPY resources/ ./resources/
COPY README.md ./

# Build the project (debug mode only)
RUN cargo build --all-features && \
    cargo test --all-features --no-run 