# syntax=docker/dockerfile:1.4
# --- Stage 1: Chef ---
FROM rust:alpine3.21 AS chef
# Install cargo-chef
RUN apk add --no-cache musl-dev gcc make && \
    cargo install cargo-chef --version 0.1.71

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
RUN --mount=type=cache,target=/usr/local/cargo/registry,id=cargo-registry \
    --mount=type=cache,target=/usr/local/cargo/git,id=cargo-git \
    --mount=type=cache,target=/app/target,id=cargo-target \
    cargo chef cook --release --recipe-path recipe.json

# First copy just what we need for compilation (order matters for caching)
COPY Cargo.toml Cargo.lock ./

# Build application while the _source_ is bind‑mounted (not copied), so
# touching .rs files does NOT trash the Docker layer cache.
#
RUN --mount=type=bind,target=/app/src,source=./src,readonly \
    --mount=type=bind,target=/app/resources,source=./resources,readonly \
    --mount=type=cache,target=/usr/local/cargo/registry,id=cargo-registry \
    --mount=type=cache,target=/usr/local/cargo/git,id=cargo-git \
    --mount=type=cache,target=/app/target,id=cargo-target \
    cargo build --release --all-features && \
    cp target/release/dela /app/