# Use official Rust image to build the binary
FROM rust:1.80-slim-bullseye AS builder

WORKDIR /app
# We need system dependencies for some rust crates (e.g. SQLite, pkg-config, libssl-dev)
RUN apt-get update && apt-get install -y pkg-config libssl-dev libsqlite3-dev sqlite3

COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Build the release binary
RUN cargo build --release

#======================================
# Runtime Stage
#======================================
FROM debian:bullseye-slim

WORKDIR /app

# Install system dependencies
# sqlite3 is required for the database, ca-certificates for ssl requests
RUN apt-get update && apt-get install -y \
    sqlite3 \
    libsqlite3-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy rust binary from builder
COPY --from=builder /app/target/release/fitness_journal /app/fitness_journal

# Set execute permissions
RUN chmod +x /app/fitness_journal

# Define the entrypoint to the compiled binary
# Pass any necessary flags like --daemon or --signal via docker-compose command
ENTRYPOINT ["/app/fitness_journal"]
