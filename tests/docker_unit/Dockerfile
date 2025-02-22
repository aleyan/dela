# Build stage using common builder
FROM dela-builder AS builder

# Test environment
FROM alpine:3.21

# Install minimal required packages for testing
RUN apk add --no-cache \
    bash

# Copy the compiled binary from the builder stage
COPY --from=builder /app/target/release/dela /usr/local/bin/dela

# Entry point script will be mounted
CMD ["bash", "/home/testuser/test_unit.sh"]
