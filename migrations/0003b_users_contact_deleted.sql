-- =============================================================
-- MQ Digital Platform — 0003b (MySQL) users_contact_chk utk DELETED
-- PDP delete-account (#15): user DELETED boleh tanpa kontak (PII dikosongkan).
-- =============================================================

ALTER TABLE users DROP CHECK users_contact_chk;
ALTER TABLE users
    ADD CONSTRAINT users_contact_chk
    CHECK (status = 'DELETED' OR phone IS NOT NULL OR email IS NOT NULL);
