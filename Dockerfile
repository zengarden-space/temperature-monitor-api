# Build stage
FROM rust:1.82-slim AS builder

WORKDIR /app

# Copy manifest files
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install required dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN groupadd -r app && useradd -r -g app app

# Copy the binary from builder stage
COPY --from=builder /app/target/release/temperature-monitor-api /usr/local/bin/temperature-monitor-api

# Change ownership
RUN chown app:app /usr/local/bin/temperature-monitor-api

# Switch to app user
USER app

# Expose port
EXPOSE 3000

# Run the application
CMD ["temperature-monitor-api"]
