#!/usr/bin/env bash
# run_tests.sh — Run all tests within the Docker network
# (Requires 'backend' service to be running or startable via compose)
#
# Produces a pytest-cov coverage report in ./coverage_reports/:
#   - coverage_reports/html/index.html   (human-readable HTML)
#   - coverage_reports/coverage.xml      (machine-readable, CI-friendly)

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

if [ -d coverage_reports ]; then
    echo ""
    echo "Coverage artifacts:"
    if [ -f coverage_reports/html/index.html ]; then
        echo "  HTML  → coverage_reports/html/index.html"
    fi
    if [ -f coverage_reports/coverage.xml ]; then
        echo "  XML   → coverage_reports/coverage.xml"
    fi
fi
