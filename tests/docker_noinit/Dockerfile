# syntax=docker/dockerfile:1.4
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
    poetry \
    nodejs \
    npm \
    maven \
    gradle \
    docker-cli \
    task

# Install act (GitHub Actions runner)
# TODO: Get act from the package manager, once it is in a non-edge alpine release.
RUN apk add --no-cache curl git bash && \
    mkdir -p /tmp/act-installation && \
    cd /tmp/act-installation && \
    curl -s https://raw.githubusercontent.com/nektos/act/master/install.sh | bash && \
    mv ./bin/act /usr/local/bin/ && \
    chmod +x /usr/local/bin/act && \
    cd / && \
    rm -rf /tmp/act-installation

# Create test user
RUN adduser -D -s /bin/zsh testuser

# Create test directories and dela config
RUN mkdir -p /home/testuser/test_project /home/testuser/.dela && \
    chown -R testuser:testuser /home/testuser && \
    chmod -R 755 /home/testuser/.dela

# Create initial allowlist with only npm-test
RUN echo 'entries = [' > /home/testuser/.dela/allowlist.toml && \
    echo '  { path = "/home/testuser/test_project/package.json", scope = "Task", tasks = ["npm-test"] }' >> /home/testuser/.dela/allowlist.toml && \
    echo ']' >> /home/testuser/.dela/allowlist.toml && \
    chown testuser:testuser /home/testuser/.dela/allowlist.toml && \
    chmod 644 /home/testuser/.dela/allowlist.toml

# Create task definitions file
RUN echo "tasks:" > /home/testuser/test_project/task_definitions.yml && \
    echo "  test-task:" >> /home/testuser/test_project/task_definitions.yml && \
    echo "    cmd: echo \"Test task executed successfully\"" >> /home/testuser/test_project/task_definitions.yml && \
    echo "    description: \"A test task\"" >> /home/testuser/test_project/task_definitions.yml

# Copy test files
COPY tests/task_definitions/Makefile /home/testuser/test_project/
COPY tests/task_definitions/package.json /home/testuser/test_project/
COPY tests/task_definitions/pyproject.toml /home/testuser/test_project/
COPY tests/task_definitions/uv.lock /home/testuser/test_project/
COPY tests/task_definitions/Taskfile.yml /home/testuser/test_project/
COPY tests/task_definitions/pom.xml /home/testuser/test_project/
COPY tests/task_definitions/build.gradle /home/testuser/test_project/
COPY tests/task_definitions/build.gradle.kts /home/testuser/test_project/
# Copy GitHub Actions workflow files
COPY tests/task_definitions/github_actions/.github /home/testuser/test_project/.github
# Copy Docker Compose files
COPY tests/task_definitions/docker-compose.yml /home/testuser/test_project/
COPY tests/task_definitions/compose.yml /home/testuser/test_project/
RUN chown -R testuser:testuser /home/testuser/test_project

# Copy the compiled binary from the builder stage
COPY --from=builder /app/target/debug/dela /usr/local/bin/

USER testuser
WORKDIR /home/testuser/test_project

# Set up environment variables
ENV HOME=/home/testuser
ENV SHELL=/bin/zsh
ENV PATH="/home/testuser/.local/bin:${PATH}"

# Entry point script will be mounted
CMD ["zsh", "/home/testuser/test_script.sh"]
