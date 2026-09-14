-- =============================================================
-- MQ Digital Platform — 0001 Extensions
-- citext  : email case-insensitive
-- pg_trgm : pencarian arsip Tanya Ustadz toleran typo
-- =============================================================

CREATE EXTENSION IF NOT EXISTS citext;
CREATE EXTENSION IF NOT EXISTS pg_trgm;
