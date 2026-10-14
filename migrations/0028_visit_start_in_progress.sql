-- 0028: fitur Mulai Kunjungan — status IN_PROGRESS + kolom started_at.
-- Kill-switch aturan jadwal (min-H booking / jendela start / jendela complete) ada di env
-- MQ_VISIT_SCHEDULE_RULES_ENABLED (default true; false = mode testing).
-- Idempotent + kompat MySQL8 (dev) & MariaDB 10.11 (prod): guard information_schema + PREPARE.

SET @has_col := (SELECT COUNT(*) FROM information_schema.COLUMNS
  WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'ustadz_visits' AND COLUMN_NAME = 'started_at');
SET @sql := IF(@has_col = 0,
  'ALTER TABLE ustadz_visits ADD COLUMN started_at TIMESTAMP NULL DEFAULT NULL AFTER scheduled_at',
  'SELECT ''skip: kolom started_at sudah ada''');
PREPARE s FROM @sql; EXECUTE s; DEALLOCATE PREPARE s;

SET @has_chk := (SELECT COUNT(*) FROM information_schema.TABLE_CONSTRAINTS
  WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'ustadz_visits' AND CONSTRAINT_NAME = 'uv_status_chk');
SET @sql := IF(@has_chk = 1,
  'ALTER TABLE ustadz_visits DROP CONSTRAINT uv_status_chk',
  'SELECT ''skip: uv_status_chk tidak ada''');
PREPARE s FROM @sql; EXECUTE s; DEALLOCATE PREPARE s;

SET @sql := IF(@has_chk = 1,
  'ALTER TABLE ustadz_visits ADD CONSTRAINT uv_status_chk CHECK (status IN (''REQUESTED'',''WAITING_CONFIRM'',''CONFIRMED'',''IN_PROGRESS'',''DECLINED'',''CANCELED'',''PAYMENT_EXPIRED'',''COMPLETED'',''REVIEWED''))',
  'SELECT ''skip: tidak ada constraint lama utk diganti''');
PREPARE s FROM @sql; EXECUTE s; DEALLOCATE PREPARE s;
