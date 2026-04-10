#!/usr/bin/env bash
# run_tests.sh — Run all unit tests and API tests via Docker (no local Python required)
# The server must be running at http://localhost:8000 before executing API tests.
# Start it with: docker compose up -d

set -euo pipefail
cd "$(dirname "$0")"

PYTHON_IMAGE="python:3.12-slim"
INSTALL="pip install pytest requests -q"

echo "========================================="
echo "  Unit tests (no server required)"
echo "========================================="
docker run --rm \
  -v "$(pwd)/unit_tests:/tests/unit_tests:ro" \
  "$PYTHON_IMAGE" \
  sh -c "$INSTALL && pytest /tests/unit_tests/ -v"

echo ""
echo "========================================="
echo "  API tests (server must be running)"
echo "========================================="
docker run --rm \
  --network host \
  -v "$(pwd)/API_tests:/tests/API_tests:ro" \
  "$PYTHON_IMAGE" \
  sh -c "$INSTALL && pytest /tests/API_tests/ -v"

echo ""
echo "All tests passed."
