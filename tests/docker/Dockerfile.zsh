# --- Stage 1: Builder ---
FROM rust:alpine3.21 AS builder

# Install build dependencies
RUN apk add --no-cache \
    musl-dev \
    gcc \
    make \
    openssl-dev \
    pkgconfig

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
FROM alpine:3.21
    
# Install required packages
RUN apk add --no-cache \
    zsh \
    make \
    python3 \
    uv \
    poetry

# Create test user
RUN adduser -D -s /bin/zsh testuser

# Set up basic zsh configuration
COPY tests/docker/zshrc.test /home/testuser/.zshrc
RUN chown testuser:testuser /home/testuser/.zshrc

# Copy test files
COPY tests/docker/Makefile.test /home/testuser/Makefile
COPY tests/docker/package.json.test /home/testuser/package.json
COPY tests/docker/pyproject.toml.test /home/testuser/pyproject.toml
RUN chown -R testuser:testuser /home/testuser

# Copy the compiled binary from the builder stage
COPY --from=builder /app/target/release/dela /usr/local/bin/dela

USER testuser
WORKDIR /home/testuser

# Set up environment variables
ENV HOME=/home/testuser
ENV SHELL=/bin/zsh
ENV PATH="/home/testuser/.local/bin:${PATH}"

# Entry point script will be mounted
CMD ["zsh", "/home/testuser/test_script.sh"]