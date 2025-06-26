# --- Builder Stage ---
FROM rust:1.87 AS builder

# 1. Set the working directory
WORKDIR /usr/src/yuxi

# 2. Copy your local code
COPY . .

# 3. Build the application. The migration logic is compiled directly into the binary.
RUN cargo build --release --workspace


# --- Final Stage ---
# Use Debian 12 "Bookworm" for the correct libraries
FROM debian:bookworm-slim

# Create a non-root user for security
ARG APP_USER=appuser
RUN groupadd -r ${APP_USER} && useradd -r -g ${APP_USER} -m -s /sbin/nologin ${APP_USER}

# Set the working directory for the final image
WORKDIR /app

# 4. Copy the single compiled binary from the builder stage
COPY --from=builder /usr/src/yuxi/target/release/yuxi .

# 5. Make the binary executable and set ownership
RUN chmod +x ./yuxi && chown -R ${APP_USER}:${APP_USER} /app

# Switch to the non-root user
USER ${APP_USER}

# Expose the port your application will listen on
EXPOSE 8000

# 6. Set the entrypoint directly to your application binary
ENTRYPOINT ["./yuxi"]