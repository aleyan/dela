# Build stage using common builder
FROM dela-builder AS builder

# Test environment
FROM alpine:3.21
    
# Install required packages
RUN apk add --no-cache \
    fish \
    make \
    python3 \
    uv \
    poetry

# Create test user
RUN adduser -D -s /bin/fish testuser

# Set up fish configuration directory and file
RUN mkdir -p /home/testuser/.config/fish
COPY tests/docker_fish/config.fish.test /home/testuser/.config/fish/config.fish
RUN chown -R testuser:testuser /home/testuser/.config && \
    chmod 644 /home/testuser/.config/fish/config.fish

# Create dela directory with proper permissions
RUN mkdir -p /home/testuser/.dela && \
    chown -R testuser:testuser /home/testuser/.dela

# Copy test files
COPY tests/task_definitions /home/testuser/
RUN chown -R testuser:testuser /home/testuser

# Copy the compiled binary from the builder stage
COPY --from=builder /app/target/release/dela /usr/local/bin/dela

USER testuser
WORKDIR /home/testuser

# Set up environment variables
ENV HOME=/home/testuser
ENV SHELL=/bin/fish
ENV PATH="/home/testuser/.local/bin:${PATH}"

# Entry point script will be mounted
CMD ["fish", "/home/testuser/test_script.sh"]