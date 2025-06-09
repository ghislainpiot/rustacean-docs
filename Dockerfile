# Simple Dockerfile for Rustacean Docs MCP Server
FROM rust:1.83-alpine AS builder
WORKDIR /app

# Install build dependencies
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static

# Set static linking
ENV OPENSSL_STATIC=1

# Copy and build
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl --bin rustacean-docs-server

# Runtime
FROM alpine:3.19
RUN apk add --no-cache ca-certificates && \
    adduser -D -u 1000 rustacean && \
    mkdir -p /app/cache && \
    chown rustacean:rustacean /app/cache

# Copy binary
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/rustacean-docs-server /usr/local/bin/

USER rustacean
WORKDIR /app
EXPOSE 8000
CMD ["rustacean-docs-server"]
