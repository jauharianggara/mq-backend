-- =============================================================
-- MQ — 0023 Master Data (pengguna aplikasi santri & ustadz)
-- Idempotent: guard information_schema + PREPARE.
-- =============================================================

-- ---------- ustadz_profiles: keustadzan ----------
SET @c := (SELECT COUNT(*) FROM information_schema.columns
           WHERE table_schema=DATABASE() AND table_name='ustadz_profiles' AND column_name='pendidikan_terakhir');
SET @sql := IF(@c=0, 'ALTER TABLE ustadz_profiles ADD COLUMN pendidikan_terakhir VARCHAR(120) NULL AFTER bio', 'SELECT 1');
PREPARE s FROM @sql; EXECUTE s; DEALLOCATE PREPARE s;

SET @c := (SELECT COUNT(*) FROM information_schema.columns
           WHERE table_schema=DATABASE() AND table_name='ustadz_profiles' AND column_name='pengalaman_mengajar');
SET @sql := IF(@c=0, 'ALTER TABLE ustadz_profiles ADD COLUMN pengalaman_mengajar TEXT NULL AFTER pendidikan_terakhir', 'SELECT 1');
PREPARE s FROM @sql; EXECUTE s; DEALLOCATE PREPARE s;

-- ---------- rekening penarikan ustadz (1:1) ----------
CREATE TABLE IF NOT EXISTS ustadz_bank_accounts (
  user_id BIGINT NOT NULL PRIMARY KEY,
  bank_name VARCHAR(50) NOT NULL,
  bank_account_no VARCHAR(40) NOT NULL,
  bank_account_name VARCHAR(100) NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
