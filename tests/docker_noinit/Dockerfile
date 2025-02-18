# Build stage using common builder
FROM dela-builder AS builder

# Test environment
FROM alpine:3.21

# Install minimal required packages for testing
RUN apk add --no-cache \
    bash \
    make \
    python3 \
    py3-pip \
    uv \
    poetry \
    npm

# Create test user
RUN adduser -D -s /bin/bash testuser

# Set up basic bash configuration
COPY tests/docker_bash/bashrc.test /home/testuser/.bashrc
RUN chown testuser:testuser /home/testuser/.bashrc && \
    chmod 644 /home/testuser/.bashrc

# Copy test files
COPY tests/task_definitions /home/testuser/
RUN chown -R testuser:testuser /home/testuser

# Copy the compiled binary from the builder stage
COPY --from=builder /app/target/release/dela /usr/local/bin/dela

USER testuser
WORKDIR /home/testuser

# Set up environment variables
ENV HOME=/home/testuser
ENV SHELL=/bin/bash
ENV PATH="/home/testuser/.local/bin:${PATH}"

# Entry point script will be mounted
CMD ["bash", "/home/testuser/test_script.sh"]
