-- =============================================================
-- MQ Digital Platform — 0031 (MySQL) Outbox push FCM
--
-- Channel PUSH dikirim via pump outbox (src/infrastructure/fcm.rs):
-- setiap baris user_notifications dgn pushed_at NULL dipump ke
-- device aktif (user_devices), routing credential per app_id
-- (SANTRI_APP/USTADZ_APP). Baris eksisting dianggap selesai
-- (pump hanya menangani notifikasi baru).
-- =============================================================

ALTER TABLE user_notifications
    ADD COLUMN pushed_at TIMESTAMP NULL AFTER read_at,
    ADD COLUMN push_attempts SMALLINT NOT NULL DEFAULT 0 AFTER pushed_at;

UPDATE user_notifications SET pushed_at = UTC_TIMESTAMP();

CREATE INDEX user_notifications_push_pending_idx ON user_notifications (pushed_at, id);
