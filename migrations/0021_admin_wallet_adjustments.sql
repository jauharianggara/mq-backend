-- =============================================================
-- MQ Digital Platform — 0021 (MySQL) Penyesuaian saldo dgn ACC santri
-- (plan panggil-ustadz-v2: admin mengajukan +/- nominal + alasan wajib,
--  santri ACC di app -> saldo berubah; semuanya berjejak)
-- =============================================================

CREATE TABLE admin_wallet_adjustments (
  id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
  user_id BIGINT NOT NULL,
  admin_id BIGINT NULL,
  amount BIGINT NOT NULL,              -- bisa negatif (pengurangan saldo)
  reason VARCHAR(255) NOT NULL,
  status VARCHAR(10) NOT NULL DEFAULT 'PENDING',  -- PENDING | ACCEPTED | REJECTED
  handled_at TIMESTAMP NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  KEY awa_user_idx (user_id, status),
  CONSTRAINT awa_user_fk FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
  CONSTRAINT awa_admin_fk FOREIGN KEY (admin_id) REFERENCES users (id) ON DELETE SET NULL,
  CONSTRAINT awa_status_chk CHECK (status IN ('PENDING','ACCEPTED','REJECTED'))
) ENGINE=InnoDB;

INSERT IGNORE INTO settings (`key`, value, description) VALUES
  ('payout_fee_amount', CAST(6500 AS JSON), 'Biaya penarikan dana ustadz (Rp)'),
  ('payout_min_amount', CAST(50000 AS JSON), 'Minimum penarikan dana ustadz (Rp)'),
  ('visit_max_hours', CAST(8 AS JSON), 'Durasi maksimal kunjungan (jam)');
