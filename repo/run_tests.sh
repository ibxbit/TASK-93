#!/usr/bin/env bash
# run_tests.sh — Run all tests within the Docker network
# (Requires 'backend' service to be running or startable via compose)

set -euo pipefail
cd "$(dirname "$0")"

echo "========================================="
echo "  Running tests via Docker Compose"
echo "========================================="

# Ensure the backend is healthy before running integration-test service.
# depends_on with service_healthy in docker-compose.yml handles this.
docker compose --profile test run --rm integration-test

echo ""
echo "All tests completed."
