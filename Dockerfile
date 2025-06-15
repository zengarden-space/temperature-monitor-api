# Build stage
FROM rust:1.82-slim AS builder

WORKDIR /app

# Copy manifest files first
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies (this will be cached unless Cargo.toml/Cargo.lock changes)
RUN cargo build --release && rm src/main.rs

# Copy actual source code
COPY src ./src

# Build the application (only your code will be recompiled)
RUN rm target/*/temperature-monitor-api* && cargo build --release

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
