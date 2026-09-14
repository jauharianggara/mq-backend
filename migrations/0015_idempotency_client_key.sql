-- =============================================================
-- MQ Digital Platform — 0015 (MySQL) idempotency client_key
-- (Bagian III M1.3 — header Idempotency-Key; nullable UNIQUE tanpa marker)
-- =============================================================

ALTER TABLE memorization_submissions
    ADD COLUMN client_key VARCHAR(64) NULL AFTER note,
    ADD UNIQUE KEY ms_user_clientkey_uq (user_id, client_key);

ALTER TABLE question_messages
    ADD COLUMN client_key VARCHAR(64) NULL AFTER duration_ms,
    ADD UNIQUE KEY qm_sender_clientkey_uq (sender_id, client_key);
