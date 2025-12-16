FROM rust:1.92.0-alpine3.23 AS builder

# Install build dependencies for Rust and native libraries
RUN apk add --no-cache \
    musl-dev \
    openssl-dev \
    openssl-libs-static \
    pkgconfig \
    cmake \
    make \
    g++ \
    git \
    perl

WORKDIR /app

# Copy dependency files first for better caching
COPY Cargo.toml Cargo.lock ./

# Create a dummy src directory and main.rs to build dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs

# Build dependencies (this layer will be cached if Cargo.toml doesn't change)
RUN cargo build --release && \
    rm -rf src

# Copy actual source code
COPY src ./src

# Build the actual application
RUN touch src/main.rs && \
    cargo build --release && \
    strip target/release/example_rust_web_service

# Runtime stage - minimal Alpine image
FROM alpine:3.23

# Install only runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    libgcc \
    openssl \
    && update-ca-certificates

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/example_rust_web_service /app/app

# Create non-root user for security
RUN addgroup -g 1000 appuser && \
    adduser -D -u 1000 -G appuser appuser && \
    chown -R appuser:appuser /app

USER appuser

EXPOSE 8080

CMD ["./app"]
