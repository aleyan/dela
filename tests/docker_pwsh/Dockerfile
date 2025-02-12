# Build stage using common builder
FROM dela-builder AS builder

# Test environment
FROM alpine:3.21
    
# Install required packages
RUN apk add --no-cache \
    powershell \
    make \
    python3 \
    uv \
    poetry

# Create test user
RUN adduser -D -s /bin/pwsh testuser

# Create PowerShell profile directory and set up profile
RUN mkdir -p /home/testuser/.config/powershell && \
    chown -R testuser:testuser /home/testuser/.config

# Set up PowerShell profile
COPY --chown=testuser:testuser tests/docker_pwsh/Microsoft.PowerShell_profile.ps1 /home/testuser/.config/powershell/Microsoft.PowerShell_profile.ps1
RUN chmod 644 /home/testuser/.config/powershell/Microsoft.PowerShell_profile.ps1

# Create dela directory with proper permissions
RUN mkdir -p /home/testuser/.dela && \
    chown -R testuser:testuser /home/testuser/.dela

# Copy test files
COPY tests/task_definitions /home/testuser/
RUN chown -R testuser:testuser /home/testuser

# Set working directory
WORKDIR /home/testuser

# Copy dela binary
COPY --from=builder /app/target/release/dela /usr/local/bin/dela

# Set shell environment variable
ENV SHELL=/bin/pwsh

# Switch to test user
USER testuser

# Run the test script
CMD ["pwsh", "./test_script.ps1"]