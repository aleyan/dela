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

# Copy project files
COPY . .

# Set ownership
RUN chown -R testuser:testuser .

# Switch to test user
USER testuser

# Entry point script will be mounted
CMD ["bash", "/home/testuser/test_script.sh"]
