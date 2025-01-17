# --- Stage 1: Builder ---
FROM rust:slim-bookworm AS builder

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

# --- Stage 2: Test environment ---
FROM debian:bookworm-slim

# Install minimal required packages for testing
RUN apt-get update && apt-get install -y \
    bash \
    make \
    && rm -rf /var/lib/apt/lists/*

# Create test user
RUN useradd -m -s /bin/bash testuser

# Set up basic bash configuration
COPY --chown=testuser:testuser tests/docker_bash/bashrc.test /home/testuser/.bashrc

# Copy test Makefile
COPY tests/task_definitions /home/testuser/

USER testuser
WORKDIR /home/testuser

# Copy the compiled binary from the builder stage
COPY --from=builder --chown=testuser:testuser /app/target/release/dela /usr/local/bin/dela

# Set up test environment
ENV HOME=/home/testuser
ENV SHELL=/bin/bash

# Entry point script will be mounted
CMD ["bash", "/home/testuser/test_script.sh"] 