# Use rust:1.99.0-alpine3.24 as base image
FROM rust:1.99.0-alpine3.24

# Install necessary packages for building
RUN apk add --no-cache \
    musl-dev \
    pkgconfig \
    openssl-dev

# Set working directory
WORKDIR /app

# Copy the backend source code
COPY backend .

# Build the application in release mode
RUN cargo build --release --locked

# Expose the default port
EXPOSE 8080

# Run the application
CMD ["./target/release/backend"]