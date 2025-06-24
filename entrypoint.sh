#!/bin/sh
set -e

echo "Running database migrations..."
sea-orm-cli migrate up

echo "Starting application..."

exec ./yuxi "$@"