#!/bin/sh

# # Print all environment variables to the container's log
# echo "--- Printing environment variables at execution time ---"
# printenv
# echo "----------------------------------------------------"

# Now, execute the main application
# The 'exec' command replaces the shell process with your app's process
exec ./yuxi