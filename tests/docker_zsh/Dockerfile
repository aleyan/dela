# Build stage using common builder
FROM dela-builder AS builder

# Test environment
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
COPY tests/docker_zsh/zshrc.test /home/testuser/.zshrc
RUN chown testuser:testuser /home/testuser/.zshrc && \
    chmod 644 /home/testuser/.zshrc

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
ENV SHELL=/bin/zsh
ENV ZSH_VERSION=5.9
ENV PATH="/home/testuser/.local/bin:${PATH}"

# Entry point script will be mounted
CMD ["zsh", "/home/testuser/test_script.sh"]