#!/usr/bin/env sh
set -eu
cd "$(dirname "$0")/.."
if [ -e .env ]; then echo '.env already exists; leaving it unchanged'; exit 0; fi
umask 077
key=$(openssl rand -hex 32)
password=$(openssl rand -hex 24)
cat > .env <<EOF
NEXUS_API_KEY=$key
POSTGRES_PASSWORD=$password
DATABASE_URL=postgres://nexus:$password@localhost:5432/nexus
NEXUS_BIND=127.0.0.1:8080
RUST_LOG=info
EOF
echo 'Created .env with random local credentials. Run docker compose up --build -d.'
