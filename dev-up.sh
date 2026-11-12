#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
BACKEND_PID=""
FRONTEND_PID=""

cleanup() {
    if [ -n "${BACKEND_PID}" ] && kill -0 "${BACKEND_PID}" 2>/dev/null; then
        kill "${BACKEND_PID}"
    fi
    if [ -n "${FRONTEND_PID}" ] && kill -0 "${FRONTEND_PID}" 2>/dev/null; then
        kill "${FRONTEND_PID}"
    fi
}

trap cleanup EXIT INT TERM

echo "[dev-up] starting backend..."
(
    cd "${ROOT_DIR}/backend"
    cargo run --bin consensus-meeting
) &
BACKEND_PID=$!

echo "[dev-up] starting frontend..."
(
    cd "${ROOT_DIR}/frontend"
    pnpm dev
) &
FRONTEND_PID=$!

wait -n "${BACKEND_PID}" "${FRONTEND_PID}"
