FROM rust:1.88-slim AS builder

WORKDIR /app
# We need system dependencies for some rust crates (e.g. SQLite, pkg-config, libssl-dev)
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    sqlite3 \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Build the release binary
RUN cargo build --release

#======================================
# Runtime Stage
#======================================
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy rust binary from builder
COPY --from=builder /app/target/release/fitness_journal /app/fitness_journal

# Copy exercise database
COPY ["Garmin Exercises Database - Exercises.csv", "/app/"]

# Set execute permissions
RUN chmod +x /app/fitness_journal

# Create data directory for the SQLite database
RUN mkdir -p /app/data

# Define the entrypoint to the compiled binary
# Pass any necessary flags like --daemon or --signal via docker-compose command
ENTRYPOINT ["/app/fitness_journal"]
