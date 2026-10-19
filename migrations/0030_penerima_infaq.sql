-- =============================================================
-- MQ Digital Platform — 0030 (MySQL/MariaDB) Penerima Infaq
--
-- Penerima pembagian income kunjungan (fee infaq) dgn wallet nyata:
--   * role PENERIMA + permission infaq.view / infaq.withdraw
--   * fee & minimal penarikan terpisah dr ustadz (default: gratis, min 50rb)
-- Pembagian income tetap lewat settings visit_income_split (target USER).
-- =============================================================

INSERT INTO roles (code, name, description) VALUES
  ('PENERIMA', 'Penerima Infaq',
   'Penerima pembagian infaq: saldo, mutasi, ajukan penarikan (login panel admin)')
ON DUPLICATE KEY UPDATE name = VALUES(name), description = VALUES(description);

INSERT INTO permissions (code, module, description) VALUES
  ('infaq.view', 'infaq', 'Lihat saldo & mutasi infaq sendiri'),
  ('infaq.withdraw', 'infaq', 'Ajukan penarikan dana infaq')
ON DUPLICATE KEY UPDATE module = VALUES(module), description = VALUES(description);

INSERT IGNORE INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r
JOIN permissions p ON p.code IN ('infaq.view', 'infaq.withdraw')
WHERE r.code = 'PENERIMA';

INSERT INTO settings (`key`, value, description) VALUES
  ('payout_fee_amount_infaq', '0', 'Biaya admin penarikan penerima infaq (Rp, default gratis)'),
  ('payout_min_amount_infaq', '50000', 'Minimal penarikan penerima infaq (Rp)')
ON DUPLICATE KEY UPDATE description = VALUES(description);
