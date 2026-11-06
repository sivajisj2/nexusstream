#!/usr/bin/env sh
# Optional local fallback when running outside ChatGPT. Requires GitHub CLI.
set -eu
cd "$(dirname "$0")/.."
command -v gh >/dev/null || { echo 'Install GitHub CLI from https://cli.github.com and run gh auth login'; exit 1; }
account=$(gh api user --jq .login)
[ "$account" = sivajisj2 ] || { echo 'Authenticate GitHub CLI as sivajisj2 first'; exit 1; }
if gh repo view sivajisj2/nexusstream >/dev/null 2>&1; then
  echo 'Repository already exists. Inspect it before pushing; this script will not overwrite it.'
  exit 1
fi
if [ ! -d .git ]; then
  git init -b main
  git add .
  git commit -m "Build NexusStream durable Rust event ingestion service"
fi
gh repo create sivajisj2/nexusstream --public --source . --remote origin --push --description 'Durable Rust event ingestion with PostgreSQL, Axum, transactional delivery, an operator dashboard, Swagger UI, and integration tests'
