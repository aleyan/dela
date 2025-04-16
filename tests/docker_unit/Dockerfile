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

# Build dependencies to cache them
RUN cargo fetch

# Set ownership
RUN chown -R testuser:testuser /home/testuser && \
    chown -R testuser:testuser /usr/local/cargo

# Switch to test user
USER testuser

# Entry point script will be mounted
CMD ["bash", "/home/testuser/test_script.sh"]
