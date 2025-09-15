# syntax=docker/dockerfile:1.4
# Build stage using common builder
FROM dela-builder AS builder

# Test environment
FROM alpine:3.21
