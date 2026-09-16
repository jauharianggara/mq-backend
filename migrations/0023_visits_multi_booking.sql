-- =============================================================
-- MQ — 0023 (MySQL) Kunjungan: santri boleh beberapa pesanan aktif
-- Permintaan user 17Sep (demo): hapus invariant "max 1 booking aktif
-- per santri" (uv_active_uq + kolom active_marker dari 0017).
-- Idempotent: guard information_schema + PREPARE.
-- =============================================================

-- ---------- drop unique index uv_active_uq bila masih ada ----------
SET @i := (SELECT COUNT(*) FROM information_schema.statistics
           WHERE table_schema=DATABASE() AND table_name='ustadz_visits' AND index_name='uv_active_uq');
SET @sql := IF(@i>0, 'ALTER TABLE ustadz_visits DROP INDEX uv_active_uq', 'SELECT 1');
PREPARE s FROM @sql; EXECUTE s; DEALLOCATE PREPARE s;

-- ---------- drop kolom generated active_marker bila masih ada ----------
-- (tidak dipakai query manapun — hanya penanda index lama)
SET @c := (SELECT COUNT(*) FROM information_schema.columns
           WHERE table_schema=DATABASE() AND table_name='ustadz_visits' AND column_name='active_marker');
SET @sql := IF(@c>0, 'ALTER TABLE ustadz_visits DROP COLUMN active_marker', 'SELECT 1');
PREPARE s FROM @sql; EXECUTE s; DEALLOCATE PREPARE s;
