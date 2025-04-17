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

# Create dummy source files to compile dependencies
# This builds a skeleton with empty files that will compile the dependencies
RUN mkdir -p src && \
    echo 'fn main() { println!("Dummy!"); }' > src/main.rs && \
    find . -name "*.rs" -not -path "./src/main.rs" -exec touch {} \;

# Build dependencies for both release and debug modes
RUN cargo build --all-features && \
    cargo build --release --all-features && \
    cargo test --all-features --no-run && \
    cargo test --release --all-features --no-run

# Now copy the real source code
RUN rm -rf src
COPY src ./src
COPY resources ./resources

# Build the project in both modes, including test binaries
RUN cargo build --all-features && \
    cargo build --release --all-features && \
    cargo test --all-features --no-run && \
    cargo test --release --all-features --no-run 