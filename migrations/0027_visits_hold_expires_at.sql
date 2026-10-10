-- =============================================================
-- MQ — 0027 (MySQL/MariaDB) ustadz_visits.hold_expires_at
-- Kolom dipakai INSERT visits v2 (commit e8636e0 "hold atomik")
-- tapi terlewat dari 0022 (dibuat manual di mq_dev sesi W1).
-- Gejala prod: POST /api/v1/visits -> 500 1054 Unknown column.
-- Idempotent: guard information_schema + PREPARE (pola 0022).
-- =============================================================

SET @c := (SELECT COUNT(*) FROM information_schema.columns
           WHERE table_schema=DATABASE() AND table_name='ustadz_visits' AND column_name='hold_expires_at');
SET @sql := IF(@c=0, 'ALTER TABLE ustadz_visits ADD COLUMN hold_expires_at TIMESTAMP NULL AFTER created_at', 'SELECT 1');
PREPARE s FROM @sql; EXECUTE s; DEALLOCATE PREPARE s;
