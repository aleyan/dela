# syntax=docker/dockerfile:1.4
# --- Stage 1: Chef ---
FROM rust:alpine3.21 AS chef
# Install cargo-chef
RUN apk add --no-cache musl-dev gcc make && \
    cargo install cargo-chef --version ^0.1

# Set the working directory inside the container
WORKDIR /app

# --- Stage 2: Planner ---
FROM chef AS planner
# Install build dependencies for the planner
RUN apk add --no-cache \
    openssl-dev \
    pkgconfig

# Only copy what cargo-chef needs
COPY Cargo.toml Cargo.lock ./
COPY src src
# Create the dependency plan (only needs source code, not tests or resources)
RUN cargo chef prepare --recipe-path recipe.json

# --- Stage 3: Builder ---
FROM chef AS builder
# Install build dependencies
RUN apk add --no-cache \
    musl-dev \
    gcc \
    make \
    openssl-dev \
    pkgconfig

# Set the working directory inside the container
WORKDIR /app

# Copy the recipe from the planner
COPY --from=planner /app/recipe.json recipe.json

# Build dependencies using the recipe - critical step for caching
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=cargo-target,target=/app/target \
    cargo chef cook --release --recipe-path recipe.json

# First copy just what we need for compilation (order matters for caching)
COPY Cargo.toml Cargo.lock ./
COPY src src

# Then copy additional files needed for runtime
COPY resources resources

# Build the application with cached dependencies and all features
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=cargo-target,target=/app/target \
    cargo build --release --all-features && \
    cp target/release/dela /app/