#!/usr/bin/env bash
# run_tests.sh — Run all unit tests and API tests
# The server must be running at http://localhost:8000 before executing API tests.
# Start it with: docker compose up -d

set -euo pipefail
cd "$(dirname "$0")"

echo "========================================="
echo "  Unit tests (no server required)"
echo "========================================="
python -m pytest unit_tests/ -v

echo ""
echo "========================================="
echo "  API tests (server must be running)"
echo "========================================="
python -m pytest API_tests/ -v

echo ""
echo "All tests passed."
