FROM dela-builder AS builder

# Test environment
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

# Copy the cargo registry from the builder and fix permissions
COPY --from=builder /usr/local/cargo/registry /usr/local/cargo/registry
RUN chmod -R 777 /usr/local/cargo/registry

# Copy project files
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY resources ./resources

# Set ownership
RUN chown -R testuser:testuser .

# Switch to test user
USER testuser

# Entry point script will be mounted
CMD ["bash", "/home/testuser/test_script.sh"]
