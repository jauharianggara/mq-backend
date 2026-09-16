-- =============================================================
-- MQ Digital Platform — 0019 (MySQL) Panggil Ustadz v2:
-- ketersediaan mingguan + deposit + penarikan + durasi jam
-- (plan 2026-09-15_mq-panggil-ustadz-v2.md rev 2)
--  * ustadz_visit_settings.price_per_hour : 1 tarif/jam (jenis layanan dihapus)
--  * ustadz_availability_slots            : jadwal mingguan berulang (menit dr 00:00)
--  * ustadz_blackout_dates                : tanggal libur (menimpa mingguan)
--  * payout_requests                      : penarikan dana ustadz (hold saldo sejak ajukan)
--  * wallets + wallet_transactions        : deposit santri & penghasilan ustadz
--    (refund SEMUA = kredit deposit; Xendit Refund API tidak dipakai)
--  * ustadz_visits                        : durasi jam bulat + hold_expires_at;
--    jenis layanan dihapus total
-- =============================================================

-- 1. pengaturan ustadz: tarif per jam
ALTER TABLE ustadz_visit_settings
  ADD COLUMN price_per_hour BIGINT NOT NULL DEFAULT 10000 AFTER max_active_visits,
  ADD CONSTRAINT uvs_price_chk CHECK (price_per_hour >= 1000 AND price_per_hour <= 100000000);

-- 2. ketersediaan mingguan
CREATE TABLE ustadz_availability_slots (
  id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
  ustadz_id BIGINT NOT NULL,
  weekday TINYINT NOT NULL,            -- 0=Minggu .. 6=Sabtu
  start_minute SMALLINT NOT NULL,      -- menit dari 00:00 (960 = 16:00)
  end_minute SMALLINT NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE KEY uas_uq (ustadz_id, weekday, start_minute),
  CONSTRAINT uas_ustadz_fk FOREIGN KEY (ustadz_id) REFERENCES users (id) ON DELETE CASCADE,
  CONSTRAINT uas_chk CHECK (end_minute > start_minute AND start_minute BETWEEN 0 AND 1439 AND end_minute BETWEEN 1 AND 1440)
) ENGINE=InnoDB;

-- 3. tanggal libur (menimpa jadwal mingguan pada tanggal tsb)
CREATE TABLE ustadz_blackout_dates (
  ustadz_id BIGINT NOT NULL,
  off_date DATE NOT NULL,
  note VARCHAR(100) NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (ustadz_id, off_date),
  CONSTRAINT ubd_ustadz_fk FOREIGN KEY (ustadz_id) REFERENCES users (id) ON DELETE CASCADE
) ENGINE=InnoDB;

-- 4. penarikan dana ustadz
CREATE TABLE payout_requests (
  id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
  ustadz_id BIGINT NOT NULL,
  amount BIGINT NOT NULL,              -- diterima ustadz (setelah fee)
  fee BIGINT NOT NULL DEFAULT 0,
  bank_name VARCHAR(50) NOT NULL,
  bank_account_no VARCHAR(30) NOT NULL,
  bank_account_name VARCHAR(100) NOT NULL,
  status VARCHAR(15) NOT NULL DEFAULT 'PENDING',  -- PENDING | APPROVED | TRANSFERRED | REJECTED
  rejected_reason VARCHAR(255) NULL,
  processed_by BIGINT NULL,
  processed_at TIMESTAMP NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  KEY pr_ustadz_idx (ustadz_id, status),
  KEY pr_status_idx (status, created_at),
  CONSTRAINT pr_ustadz_fk FOREIGN KEY (ustadz_id) REFERENCES users (id) ON DELETE CASCADE,
  CONSTRAINT pr_status_chk CHECK (status IN ('PENDING','APPROVED','TRANSFERRED','REJECTED'))
) ENGINE=InnoDB;

-- 5. deposit & penghasilan (generik per user — santri & ustadz)
CREATE TABLE wallets (
  user_id BIGINT NOT NULL PRIMARY KEY,
  balance BIGINT NOT NULL DEFAULT 0,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  CONSTRAINT w_user_fk FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
  CONSTRAINT w_bal_chk CHECK (balance >= 0)
) ENGINE=InnoDB;

CREATE TABLE wallet_transactions (
  id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
  user_id BIGINT NOT NULL,
  tx_type VARCHAR(10) NOT NULL,        -- TOPUP | PAYMENT | REFUND | EARNING | PAYOUT | ADJUST
  amount BIGINT NOT NULL,              -- selalu positif; arah dari tx_type
  balance_after BIGINT NOT NULL,
  subject_type VARCHAR(30) NULL,       -- 'ustadz_visit' / 'xendit_topup'
  subject_id BIGINT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  KEY wt_user_idx (user_id, id),
  CONSTRAINT wt_user_fk FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
  CONSTRAINT wt_type_chk CHECK (tx_type IN ('TOPUP','PAYMENT','REFUND','EARNING','PAYOUT','ADJUST')),
  CONSTRAINT wt_amt_chk CHECK (amount > 0)
) ENGINE=InnoDB;

-- 6. ustadz_visits: hapus jenis layanan, tambah durasi jam & hold
ALTER TABLE ustadz_visits
  DROP FOREIGN KEY uv_type_fk,
  DROP INDEX uv_ustadz_sched_idx,
  DROP COLUMN service_type_id,
  ADD COLUMN duration_hours TINYINT NOT NULL DEFAULT 2 AFTER scheduled_at,
  ADD COLUMN hold_expires_at TIMESTAMP NULL AFTER created_at;

ALTER TABLE ustadz_visits
  DROP INDEX uv_sched_idx,
  ADD INDEX uv_sched_idx (status, scheduled_at);

-- 7. bersihkan tabel jenis layanan
DROP TABLE ustadz_visit_services;
DROP TABLE visit_service_types;
