FROM rust:alpine3.21

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

# Set ownership
RUN chown -R testuser:testuser .

# Switch to test user
USER testuser

# Copy Cargo definition files first (better Docker caching)
COPY Cargo.toml Cargo.lock ./

# Pre-fetch dependencies (creates a build cache layer)
RUN cargo fetch || true

# Now copy source code and build
COPY src ./src
COPY resources ./resources

# Entry point script will be mounted
CMD ["bash", "/home/testuser/test_script.sh"]
