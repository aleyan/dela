# syntax=docker/dockerfile:1.4
# Build stage using common builder
FROM dela-builder AS builder

# Test environment
FROM debian:bookworm-slim

# Install required packages
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    git \
    build-essential \
    pkg-config \
    libssl-dev \
    make \
    nodejs \
    npm \
    && rm -rf /var/lib/apt/lists/*

# Install pnpm (not needed for simplified tests, but keeping for potential future use)
RUN npm install -g pnpm@9

# Create test user
RUN useradd -m -s /bin/bash testuser

# Create test directories
RUN mkdir -p /home/testuser/test_project /home/testuser/.dela && \
    chown -R testuser:testuser /home/testuser && \
    chmod -R 755 /home/testuser/.dela

# Create initial allowlist
RUN echo 'entries = [' > /home/testuser/.dela/allowlist.toml && \
    echo '  { path = "/home/testuser/test_project/package.json", scope = "Task", tasks = ["npm-test", "npm-start"] }' >> /home/testuser/.dela/allowlist.toml && \
    echo '  { path = "/home/testuser/test_project/Makefile", scope = "Task", tasks = ["make-build", "make-test"] }' >> /home/testuser/.dela/allowlist.toml && \
    echo ']' >> /home/testuser/.dela/allowlist.toml && \
    chown testuser:testuser /home/testuser/.dela/allowlist.toml && \
    chmod 644 /home/testuser/.dela/allowlist.toml

# Copy the compiled binary from the builder stage
COPY --from=builder /app/target/debug/dela /usr/local/bin/

# Copy task definitions from existing test fixtures
COPY tests/task_definitions/ /home/testuser/test_project/
RUN chown -R testuser:testuser /home/testuser/test_project

# Copy MCP test files
COPY tests/docker_mcp/ /home/testuser/mcp_tests/
RUN chown -R testuser:testuser /home/testuser/mcp_tests

USER testuser
WORKDIR /home/testuser/mcp_tests

# Set up environment variables
ENV HOME=/home/testuser
ENV SHELL=/bin/bash
ENV PATH="/home/testuser/.local/bin:${PATH}"

# Entry point script will be mounted
CMD ["bash", "/home/testuser/test_script.sh"]
