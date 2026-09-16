-- =============================================================
-- MQ — 0022 (MySQL) Panggil Ustadz v2: kolom tarif/jam & total di visits
-- (melengkapi sesi W1 yang meng-alter mq_dev manual tanpa file migration)
-- Idempotent: guard information_schema + PREPARE (aman di DB yang sudah
-- terlanjur setengah-alter MAUPUN fresh deploy dari 0017 v1).
-- =============================================================

-- ---------- helper guard: ADD / DROP kolom hanya bila perlu ----------
SET @c := (SELECT COUNT(*) FROM information_schema.columns
           WHERE table_schema=DATABASE() AND table_name='ustadz_visits' AND column_name='price_per_hour');
SET @sql := IF(@c=0, 'ALTER TABLE ustadz_visits ADD COLUMN price_per_hour BIGINT NOT NULL DEFAULT 0 AFTER duration_hours', 'SELECT 1');
PREPARE s FROM @sql; EXECUTE s; DEALLOCATE PREPARE s;

SET @c := (SELECT COUNT(*) FROM information_schema.columns
           WHERE table_schema=DATABASE() AND table_name='ustadz_visits' AND column_name='price_total');
SET @sql := IF(@c=0, 'ALTER TABLE ustadz_visits ADD COLUMN price_total BIGINT NOT NULL DEFAULT 0 AFTER price_per_hour', 'SELECT 1');
PREPARE s FROM @sql; EXECUTE s; DEALLOCATE PREPARE s;

-- backfill dari skema v1 (price_amount = total lama) — no-op bila sudah v2
UPDATE ustadz_visits
   SET price_total   = price_amount,
       price_per_hour = GREATEST(ROUND(price_amount / GREATEST(duration_hours, 1)), 1000)
 WHERE price_total = 0 AND price_amount > 0;

-- ---------- kolom v1 legacy: drop bila masih ada ----------
SET @c := (SELECT COUNT(*) FROM information_schema.columns
           WHERE table_schema=DATABASE() AND table_name='ustadz_visits' AND column_name='price_amount');
SET @sql := IF(@c>0, 'ALTER TABLE ustadz_visits DROP COLUMN price_amount', 'SELECT 1');
PREPARE s FROM @sql; EXECUTE s; DEALLOCATE PREPARE s;

SET @c := (SELECT COUNT(*) FROM information_schema.columns
           WHERE table_schema=DATABASE() AND table_name='ustadz_visits' AND column_name='duration_minutes');
SET @sql := IF(@c>0, 'ALTER TABLE ustadz_visits DROP COLUMN duration_minutes', 'SELECT 1');
PREPARE s FROM @sql; EXECUTE s; DEALLOCATE PREPARE s;

SET @c := (SELECT COUNT(*) FROM information_schema.columns
           WHERE table_schema=DATABASE() AND table_name='ustadz_visits' AND column_name='service_type_id');
SET @sql := IF(@c>0, 'ALTER TABLE ustadz_visits DROP FOREIGN KEY uv_type_fk, DROP COLUMN service_type_id', 'SELECT 1');
PREPARE s FROM @sql; EXECUTE s; DEALLOCATE PREPARE s;

-- tabel layanan v1 DIHAPUS (jenis layanan dihapus total — ustadz 1 tarif/jam)
DROP TABLE IF EXISTS ustadz_visit_services;
DROP TABLE IF EXISTS visit_service_types;

-- ---------- ustadz_visit_settings.price_per_hour (v2) ----------
SET @c := (SELECT COUNT(*) FROM information_schema.columns
           WHERE table_schema=DATABASE() AND table_name='ustadz_visit_settings' AND column_name='price_per_hour');
SET @sql := IF(@c=0, 'ALTER TABLE ustadz_visit_settings ADD COLUMN price_per_hour BIGINT NOT NULL DEFAULT 10000 AFTER max_active_visits, ADD CONSTRAINT uvs_price_chk CHECK (price_per_hour >= 1000 AND price_per_hour <= 100000000)', 'SELECT 1');
PREPARE s FROM @sql; EXECUTE s; DEALLOCATE PREPARE s;

-- ---------- tabel v2 (wallet / ketersediaan / payout) utk fresh deploy ----------
-- (di mq_dev dibuat manual sesi W1 — IF NOT EXISTS = no-op di sana)
CREATE TABLE IF NOT EXISTS wallets (
  user_id BIGINT PRIMARY KEY,
  balance BIGINT NOT NULL DEFAULT 0,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  CONSTRAINT w_user_fk FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
  CONSTRAINT w_bal_chk CHECK (balance >= 0)
) ENGINE=InnoDB;

CREATE TABLE IF NOT EXISTS wallet_transactions (
  id BIGINT AUTO_INCREMENT PRIMARY KEY,
  user_id BIGINT NOT NULL,
  tx_type VARCHAR(10) NOT NULL,
  amount BIGINT NOT NULL,
  balance_after BIGINT NOT NULL,
  subject_type VARCHAR(30) NULL,
  subject_id BIGINT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  CONSTRAINT wt_user_fk FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
  CONSTRAINT wt_type_chk CHECK (tx_type IN ('TOPUP','PAYMENT','REFUND','EARNING','PAYOUT','ADJUST')),
  CONSTRAINT wt_amt_chk CHECK (amount > 0),
  KEY wt_user_idx (user_id, id)
) ENGINE=InnoDB;

CREATE TABLE IF NOT EXISTS payout_requests (
  id BIGINT AUTO_INCREMENT PRIMARY KEY,
  ustadz_id BIGINT NOT NULL,
  amount BIGINT NOT NULL,
  fee BIGINT NOT NULL DEFAULT 0,
  bank_name VARCHAR(50) NOT NULL,
  bank_account_no VARCHAR(30) NOT NULL,
  bank_account_name VARCHAR(100) NOT NULL,
  status VARCHAR(15) NOT NULL DEFAULT 'PENDING',
  rejected_reason VARCHAR(255) NULL,
  processed_by BIGINT NULL,
  processed_at TIMESTAMP NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  CONSTRAINT pr_ustadz_fk FOREIGN KEY (ustadz_id) REFERENCES users(id) ON DELETE CASCADE,
  CONSTRAINT pr_status_chk CHECK (status IN ('PENDING','APPROVED','TRANSFERRED','REJECTED')),
  KEY pr_ustadz_idx (ustadz_id, status),
  KEY pr_status_idx (status, created_at)
) ENGINE=InnoDB;

CREATE TABLE IF NOT EXISTS ustadz_availability_slots (
  id BIGINT AUTO_INCREMENT PRIMARY KEY,
  ustadz_id BIGINT NOT NULL,
  weekday TINYINT NOT NULL,
  start_minute SMALLINT NOT NULL,
  end_minute SMALLINT NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  CONSTRAINT uas_uq UNIQUE (ustadz_id, weekday, start_minute),
  CONSTRAINT uas_ustadz_fk FOREIGN KEY (ustadz_id) REFERENCES users(id) ON DELETE CASCADE,
  CONSTRAINT uas_chk CHECK (end_minute > start_minute AND start_minute BETWEEN 0 AND 1439 AND end_minute BETWEEN 1 AND 1440)
) ENGINE=InnoDB;

CREATE TABLE IF NOT EXISTS ustadz_blackout_dates (
  ustadz_id BIGINT NOT NULL,
  off_date DATE NOT NULL,
  note VARCHAR(100) NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (ustadz_id, off_date),
  CONSTRAINT ubd_ustadz_fk FOREIGN KEY (ustadz_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB;

-- ---------- settings v2 (INSERT IGNORE = no-op bila sudah ada) ----------
INSERT IGNORE INTO settings (`key`, value, description) VALUES
  ('visit_min_schedule_hours', CAST(2 AS JSON), 'Jadwal minimal berapa jam ke depan'),
  ('visit_max_schedule_days',  CAST(14 AS JSON), 'Jadwal maksimal berapa hari ke depan'),
  ('visit_invoice_duration_sec', CAST(7200 AS JSON), 'Masa hidup invoice (hold slot) detik'),
  ('visit_cancel_free_hours', CAST(2 AS JSON), 'Pembatalan gratis sampai N jam sebelum jadwal'),
  ('visit_minor_booking_policy', CAST('"BLOCK"' AS JSON), 'Kebijakan booking usia < 18 th');
