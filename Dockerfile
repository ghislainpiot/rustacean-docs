# Optimized Dockerfile for Rustacean Docs MCP Server
FROM rust:1.83-alpine AS chef
# Install build dependencies needed for cargo-chef
RUN apk add --no-cache musl-dev
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
WORKDIR /app

# Install build dependencies
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static

# Set static linking
ENV OPENSSL_STATIC=1

# Build dependencies - this layer will be cached unless dependencies change
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json

# Copy source code and build application - this layer rebuilds only when source changes
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl --bin rustacean-docs-server --locked

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
