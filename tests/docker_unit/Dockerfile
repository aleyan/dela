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
    pkgconfig \
    bash

# Only copy what cargo-chef needs
COPY Cargo.toml Cargo.lock ./
COPY src src
# Create the dependency plan (only needs source code, not tests or resources)
RUN cargo chef prepare --recipe-path recipe.json

# --- Stage 3: Test Builder ---
FROM chef AS builder
# Install build dependencies
RUN apk add --no-cache \
    musl-dev \
    gcc \
    make \
    openssl-dev \
    pkgconfig \
    bash

# Create test user and directory
RUN adduser -D testuser
WORKDIR /home/testuser

# Copy the recipe from the planner
COPY --from=planner /app/recipe.json recipe.json

# Build dependencies using the recipe - critical step for caching
RUN --mount=type=cache,target=/usr/local/cargo/registry,id=cargo-registry \
    --mount=type=cache,target=/usr/local/cargo/git,id=cargo-git \
    --mount=type=cache,target=/home/testuser/target,id=cargo-target,sharing=locked \
    cargo chef cook --tests --recipe-path recipe.json

# Now copy the real source code
COPY Cargo.toml Cargo.lock ./
COPY src src
COPY resources resources

# Build and prepare for tests
# Using the same cache identifiers consistently with GitHub Actions
RUN --mount=type=cache,target=/usr/local/cargo/registry,id=cargo-registry \
    --mount=type=cache,target=/usr/local/cargo/git,id=cargo-git \
    --mount=type=cache,target=/home/testuser/target,id=cargo-target,sharing=locked \
    cargo test --no-run --all-features

# Copy test script directly into the image
COPY tests/docker_unit/test_unit.sh /home/testuser/test_script.sh
RUN chmod +x /home/testuser/test_script.sh

# Set ownership
RUN chown -R testuser:testuser /home/testuser && \
    chown -R testuser:testuser /usr/local/cargo

# Switch to test user
USER testuser

# Entry point script is now embedded in the image
CMD ["bash", "/home/testuser/test_script.sh"]
