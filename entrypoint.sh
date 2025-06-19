#!/bin/sh
set -e # Exit immediately if a command exits with a non-zero status.

# The DATABASE_URL environment variable is expected to be set by Azure.
echo "Running database migrations..."
sea-orm-cli migrate up

# The PORT (or WEBSITES_PORT for Azure App Service) environment variable
# is expected to be set by Azure. Your Rust application needs to read this
# variable to know which port to listen on.
echo "Starting application..."

# Execute your compiled Rust application.
# "$@" passes any arguments from Docker's CMD to your application.
exec ./your_main_executable_name "$@"