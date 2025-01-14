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
    zsh \
    make \
    && rm -rf /var/lib/apt/lists/*

# Create test user
RUN useradd -m -s /bin/zsh testuser

# Set up basic zsh configuration
RUN echo "ZDOTDIR=\$HOME" > /etc/zsh/zshenv
COPY --chown=testuser:testuser tests/docker/zshrc.test /home/testuser/.zshrc

# Copy test Makefile
COPY --chown=testuser:testuser tests/docker/Makefile.test /home/testuser/Makefile

# Copy test package.json
COPY --chown=testuser:testuser tests/docker/package.json.test /home/testuser/package.json

USER testuser
WORKDIR /home/testuser

# Copy the compiled binary from the builder stage
COPY --from=builder --chown=testuser:testuser /app/target/release/dela /usr/local/bin/dela

# Set up test environment
ENV HOME=/home/testuser
ENV SHELL=/bin/zsh

# Entry point script will be mounted
CMD ["zsh", "/home/testuser/test_script.sh"] 