#!/usr/bin/env bash
# Deploy both testing.jogga.fit (fedisport) and single.jogga.fit (jogga/core)
# to the shared GCP VM.
set -euo pipefail

PROJECT="project-ce74a06d-c017-4712-8f0"
ZONE="us-central1-a"
INSTANCE="jogga-testing"
JOGGA_IMAGE="us-central1-docker.pkg.dev/${PROJECT}/jogga/jogga:latest"
FEDISPORT_IMAGE="us-central1-docker.pkg.dev/${PROJECT}/fedisport/fedisport:latest"
REMOTE_DIR="~/jogga-deploy"

# ── 1. Build jogga (core) image ───────────────────────────────────────────────
echo "==> Building jogga image via Cloud Build..."
gcloud builds submit \
  --project="$PROJECT" \
  --config=cloudbuild.yaml \
  ..

# ── 2. Copy deploy files to VM ────────────────────────────────────────────────
echo "==> Copying deploy files to VM..."
gcloud compute ssh "$INSTANCE" --zone="$ZONE" --project="$PROJECT" -- \
  "mkdir -p $REMOTE_DIR"

# .env.single must exist locally (copy from .env.single.example and fill in)
if [[ ! -f .env.single ]]; then
  echo "ERROR: .env.single not found. Copy .env.single.example and fill in values."
  exit 1
fi

# .env.testing comes from the fedisport repo (existing deploy)
if [[ ! -f .env.testing ]]; then
  echo "ERROR: .env.testing not found."
  exit 1
fi

gcloud compute scp \
  docker-compose.yml Caddyfile init-db.sql .env.testing .env.single \
  "${INSTANCE}:${REMOTE_DIR}/" \
  --zone="$ZONE" --project="$PROJECT"

# ── 3. Start stack on VM ──────────────────────────────────────────────────────
echo "==> Starting stack on VM..."
gcloud compute ssh "$INSTANCE" --zone="$ZONE" --project="$PROJECT" -- "
  set -e
  gcloud auth configure-docker us-central1-docker.pkg.dev --quiet

  cd $REMOTE_DIR

  # Create jogga DB if postgres is already running (init-db.sql only runs on
  # first postgres start; existing deployments need this manual step).
  if docker compose ps postgres 2>/dev/null | grep -q 'running'; then
    echo '==> Ensuring jogga database exists...'
    docker compose exec postgres psql -U fedisport -tc \
      \"SELECT 1 FROM pg_database WHERE datname='jogga'\" | grep -q 1 || \
    docker compose exec postgres psql -U fedisport -f \
      /docker-entrypoint-initdb.d/10-jogga.sql
  fi

  docker compose pull fedisport jogga
  docker compose up -d
  docker compose ps
"

echo "==> Done."
echo "    testing.jogga.fit — fedisport (multi-user)"
echo "    single.jogga.fit  — jogga/core (single-user)"
echo "    Caddy will provision TLS certs within ~60s on first run."
