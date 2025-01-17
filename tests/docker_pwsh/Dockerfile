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

# Install prerequisites and PowerShell
RUN apt-get update && \
    apt-get install -y wget make gnupg2 && \
    # Get Debian version
    . /etc/os-release && \
    # Download and register Microsoft repository
    wget -q https://packages.microsoft.com/config/debian/12/packages-microsoft-prod.deb && \
    dpkg -i packages-microsoft-prod.deb && \
    rm packages-microsoft-prod.deb && \
    # Update package list and install PowerShell
    apt-get update && \
    apt-get install -y powershell && \
    # Clean up
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# Create test user
RUN useradd -m -s /usr/bin/pwsh testuser

# Create PowerShell profile directory and set up profile
RUN mkdir -p /home/testuser/.config/powershell && \
    chown -R testuser:testuser /home/testuser/.config

# Set up PowerShell profile
COPY --chown=testuser:testuser tests/docker/Microsoft.PowerShell_profile.ps1 /home/testuser/.config/powershell/Microsoft.PowerShell_profile.ps1

# Copy test Makefile
COPY --chown=testuser:testuser tests/docker/Makefile.test /home/testuser/Makefile
COPY --chown=testuser:testuser tests/docker/package.json.test /home/testuser/package.json

USER testuser
WORKDIR /home/testuser

# Copy the compiled binary from the builder stage
COPY --from=builder --chown=testuser:testuser /app/target/release/dela /usr/local/bin/dela

# Set up test environment
ENV HOME=/home/testuser
ENV SHELL=/usr/bin/pwsh

# Entry point script will be mounted
CMD ["pwsh", "-NoLogo", "-NoProfile", "-File", "/home/testuser/test_script.ps1"] 