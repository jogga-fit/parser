-- Creates the jogga database and user for single.jogga.fit.
-- Postgres runs this on first init (docker-entrypoint-initdb.d).
-- For existing deployments, run manually:
--   docker compose exec postgres psql -U fedisport -f /docker-entrypoint-initdb.d/10-jogga.sql

CREATE USER jogga WITH PASSWORD 'jogga';
CREATE DATABASE jogga OWNER jogga;
