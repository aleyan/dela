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
RUN apt-get update && \
    apt-get install -y \
        fish \
        make && \
    rm -rf /var/lib/apt/lists/*

# Create test user and set up fish
RUN useradd -m -s /usr/bin/fish testuser

# Set up fish configuration
RUN mkdir -p /home/testuser/.config/fish
COPY --chown=testuser:testuser tests/docker/config.fish.test /home/testuser/.config/fish/config.fish

# Copy test Makefile
COPY --chown=testuser:testuser tests/docker/Makefile.test /home/testuser/Makefile

USER testuser
WORKDIR /home/testuser

# Copy the compiled binary from the builder stage
COPY --from=builder --chown=testuser:testuser /app/target/release/dela /usr/local/bin/dela

# Set up test environment
ENV HOME=/home/testuser
ENV SHELL=/usr/bin/fish

# Entry point script will be mounted
CMD ["fish", "/home/testuser/test_script.sh"] 