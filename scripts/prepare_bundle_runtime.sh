#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PY_VENDOR_DIR="${ROOT_DIR}/runtime/python-vendor"
NODE_RUNTIME_DIR="${ROOT_DIR}/runtime/node"
BIN_RUNTIME_DIR="${ROOT_DIR}/runtime/bin"
NPM_CACHE_DIR="${ROOT_DIR}/runtime/.npm-cache"

if [[ -z "${PYTHON_BIN:-}" ]]; then
  for candidate in "/usr/bin/python3" "/opt/homebrew/bin/python3" "/usr/local/bin/python3" "python3"; do
    if command -v "${candidate}" >/dev/null 2>&1; then
      PYTHON_BIN="${candidate}"
      break
    fi
  done
fi

if [[ -z "${NODE_BIN:-}" ]]; then
  NODE_BIN="$(command -v node || true)"
fi
if [[ -z "${NODE_BIN}" || ! -x "${NODE_BIN}" ]]; then
  echo "未找到 node 可执行文件，无法准备 AI Agent 运行时。" >&2
  exit 1
fi

# 运行时依赖是生成产物，每次重建可以避免残留旧 Python ABI 或旧 npm 包。
rm -rf "${PY_VENDOR_DIR}" "${NODE_RUNTIME_DIR}/node_modules" "${BIN_RUNTIME_DIR}/node"
mkdir -p "${PY_VENDOR_DIR}"
mkdir -p "${NODE_RUNTIME_DIR}"
mkdir -p "${BIN_RUNTIME_DIR}"
mkdir -p "${NPM_CACHE_DIR}"

echo "使用 Python: $("${PYTHON_BIN}" --version)"
echo "使用 Node: $("${NODE_BIN}" --version)"
"${PYTHON_BIN}" -m pip install --upgrade --no-warn-script-location --target "${PY_VENDOR_DIR}" -r "${ROOT_DIR}/requirements.txt"
npm install --omit=dev --prefix "${NODE_RUNTIME_DIR}" --cache "${NPM_CACHE_DIR}"
cp "${NODE_BIN}" "${BIN_RUNTIME_DIR}/node"
chmod 755 "${BIN_RUNTIME_DIR}/node"

echo "运行时依赖已准备到 runtime/，Tauri 打包会一并带入安装包。"
