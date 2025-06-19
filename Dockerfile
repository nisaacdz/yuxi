# ---- STAGE 1: Builder ----
# Use a specific Rust version for reproducibility or 'rust:latest' if you prefer
FROM rust:1.77 AS builder
# FROM rust:latest AS builder

WORKDIR /usr/src/app

# Copy the entire project. Ensure .dockerignore excludes target/, .git/, etc.
COPY . .

# Install sea-orm-cli. This will be available for the build stage and can be copied to runner.
# The binary is typically installed in /root/.cargo/bin/ when running as root
RUN cargo install sea-orm-cli

# Build your application.
# If you have a single binary target in your workspace, you can be specific:
# RUN cargo build --release -p <your_main_crate_name>
# Otherwise, build all runnable binaries in the workspace:
RUN cargo build --release --workspace

# ---- STAGE 2: Runner ----
# Use a minimal base image like Debian Slim
FROM debian:bullseye-slim

# Create a non-root user for security
ARG APP_USER=appuser
RUN groupadd -r ${APP_USER} && useradd -r -g ${APP_USER} -m -s /sbin/nologin ${APP_USER}
# -m creates home directory, -s /sbin/nologin for non-interactive user

WORKDIR /app

# Copy the compiled main application binary from the builder stage
# IMPORTANT: Replace `<your_main_executable_name>` with the actual name of your executable
# This name is usually the name of your crate if it's a binary crate.
COPY --from=builder /usr/src/app/target/release/<your_main_executable_name> ./your_main_executable_name

# Copy sea-orm-cli from the builder stage
COPY --from=builder /root/.cargo/bin/sea-orm-cli /usr/local/bin/sea-orm-cli

# Copy the entrypoint script
COPY entrypoint.sh .

# Ensure executables and script have execute permissions and correct ownership
RUN chmod +x ./your_main_executable_name /usr/local/bin/sea-orm-cli ./entrypoint.sh && \
    chown -R ${APP_USER}:${APP_USER} /app

# Switch to the non-root user
USER ${APP_USER}

# --- Port Handling ---
# The EXPOSE instruction is documentation for which port the application *inside* the container
# is *intended* to listen on. It doesn't actually publish the port.
# Your Rust application MUST read an environment variable (e.g., PORT or WEBSITES_PORT from Azure)
# and listen on 0.0.0.0:<that_port>.
# Let's assume your app will listen on a port defined by the 'PORT' env var, defaulting to 8000.
# Azure App Service will set WEBSITES_PORT (e.g., 8080) and expect your app to listen there.
# So, your app should prioritize WEBSITES_PORT if present, then PORT, then a default.
# We EXPOSE a common default here, but your app's behavior is what matters.
EXPOSE 8000

# Set the entrypoint to our script
ENTRYPOINT ["./entrypoint.sh"]

# CMD is not strictly needed if your_main_executable_name takes no args,
# but can be used to pass default arguments to it if entrypoint.sh uses "$@"
# CMD ["--default-app-arg"]