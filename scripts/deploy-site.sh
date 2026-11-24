#!/usr/bin/env bash
# Publish the docapp landing page (web/ from GitHub main) into nginx.
# Usage: ./scripts/deploy-site.sh
set -euo pipefail

HOSTING_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
if [[ -d "/home/wcp/hosting" ]]; then
  HOSTING_ROOT="/home/wcp/hosting"
fi
APP_DIR="${HOSTING_ROOT}/apps/docapp"
CHECKOUT="${APP_DIR}/checkout"
WWW="${HOSTING_ROOT}/php/www/docapp"
REPO_URL="git@github.com:Emmanue707/docapp.git"

mkdir -p "${APP_DIR}"
if [[ ! -d "${CHECKOUT}/.git" ]]; then
    git clone --depth 1 --branch main "${REPO_URL}" "${CHECKOUT}"
else
    git -C "${CHECKOUT}" fetch --depth 1 origin main
    git -C "${CHECKOUT}" checkout -f main
    git -C "${CHECKOUT}" reset --hard origin/main
fi

SHA="$(git -C "${CHECKOUT}" rev-parse HEAD)"
echo "${SHA}" > "${APP_DIR}/CURRENT_SHA"

mkdir -p "${WWW}"
rsync -a --delete "${CHECKOUT}/web/" "${WWW}/"
docker exec hosting-nginx nginx -t
docker exec hosting-nginx nginx -s reload
echo "deploy-site done sha=${SHA}"