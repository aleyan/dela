# syntax=docker/dockerfile:1.4
FROM rust:alpine3.21

# Install dependencies
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

# Copy source code
COPY Cargo.toml Cargo.lock ./
COPY src src
COPY resources resources

# Build dependencies with proper caching
RUN --mount=type=cache,target=/usr/local/cargo/registry,id=unit-cargo-registry \
    --mount=type=cache,target=/usr/local/cargo/git,id=unit-cargo-git \
    --mount=type=cache,target=/home/testuser/target,id=unit-cargo-target \
    cargo fetch && \
    cargo test --no-run --all-features

# Set ownership
RUN chown -R testuser:testuser /home/testuser && \
    chown -R testuser:testuser /usr/local/cargo

# Switch to test user
USER testuser

# Entry point script will be mounted
CMD ["bash", "/home/testuser/test_script.sh"]
