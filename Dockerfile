# --- Builder Stage ---
FROM rust:1.87 AS builder

# 1. Set the working directory in the builder
WORKDIR /usr/src/yuxi

# 2. Copy all your local code into the builder's working directory
COPY . .

# 3. Install the migration tool
RUN cargo install sea-orm-cli

# 4. Build the application. The output will be at /usr/src/yuxi/target/release/yuxi
RUN cargo build --release --workspace


# --- Final Stage ---
FROM debian:bullseye-slim

# Create a non-root user to run the application
ARG APP_USER=appuser
RUN groupadd -r ${APP_USER} && useradd -r -g ${APP_USER} -m -s /sbin/nologin ${APP_USER}

# Set the working directory for the final image
WORKDIR /yuxi

# 5. [FIXED] Copy the compiled binary from the correct path in the builder stage
COPY --from=builder /usr/src/yuxi/target/release/yuxi ./yuxi

# 6. Copy the migration tool and the entrypoint script
COPY --from=builder /root/.cargo/bin/sea-orm-cli /usr/local/bin/sea-orm-cli
COPY entrypoint.sh .

# 7. [FIXED] Make files executable and set ownership on the correct directory (/yuxi)
RUN chmod +x ./yuxi /usr/local/bin/sea-orm-cli ./entrypoint.sh && \
    chown -R ${APP_USER}:${APP_USER} /yuxi

# Switch to the non-root user
USER ${APP_USER}

# Expose the port your application will listen on
EXPOSE 8000

# Set the entrypoint to your script
ENTRYPOINT ["./entrypoint.sh"]