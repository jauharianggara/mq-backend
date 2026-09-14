-- =============================================================
-- MQ Digital Platform — 0001 (MySQL) — pendahuluan konversi
-- Asal: PostgreSQL 0001 extensions (citext + pg_trgm).
-- MySQL 8 tidak membutuhkan extension:
--   * citext  -> collation DB `utf8mb4_0900_ai_ci` (case-insensitive)
--   * pg_trgm -> diganti FULLTEXT INDEX + fallback LIKE (lihat 0010)
-- File ini = no-op penanda (versi/keterangan konversi Task 1.0).
-- =============================================================

-- no-op
