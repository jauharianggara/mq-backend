-- =============================================================
-- MQ Digital Platform — 0003b (MySQL) users_contact_chk utk DELETED
-- PDP delete-account (#15): user DELETED boleh tanpa kontak (PII dikosongkan).
-- =============================================================

-- CATATAN: pakai DROP CONSTRAINT (kompatibel MariaDB 10.2+ & MySQL 8.0.19+; 'DROP CHECK' = MySQL-8-only → gagal di MariaDB)
ALTER TABLE users DROP CONSTRAINT users_contact_chk;
ALTER TABLE users
    ADD CONSTRAINT users_contact_chk
    CHECK (status = 'DELETED' OR phone IS NOT NULL OR email IS NOT NULL);
