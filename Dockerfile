# --- Builder Stage ---
FROM rust:1.86 AS builder

# Set the working directory
WORKDIR /usr/src/yuxi

# Copy your local code to build
COPY . .

# Build the application
RUN cargo build --release

# --- Final Stage ---
# Use Debian 12 "Bookworm" for the correct libraries
FROM debian:bookworm

# Create a non-root user for security
ARG APP_USER=appuser
RUN groupadd -r ${APP_USER} && useradd -r -g ${APP_USER} -m -s /sbin/nologin ${APP_USER}

# Set the working directory for the final image
WORKDIR /app

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/yuxi/target/release/yuxi .

# Copy the entrypoint script from your local machine ----
COPY entrypoint.sh .

# Set permissions and ownership for the files you've copied
RUN chmod +x ./yuxi ./entrypoint.sh && \
    chown -R ${APP_USER}:${APP_USER} /app

# Switch to the non-root user
USER ${APP_USER}

# Expose the port your application will listen on
EXPOSE 8000

# Set the entrypoint to the script
ENTRYPOINT ["./entrypoint.sh"]