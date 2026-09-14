-- =============================================================
-- MQ Digital Platform — 0012 Notification
-- MVP: channel IN_APP + PUSH (FCM). EMAIL/WHATSAPP disiapkan enum-nya,
-- pengirimnya menyusul. Push token disimpan di user_devices (0003),
-- tidak ada tabel device_tokens terpisah.
-- =============================================================

CREATE TABLE notification_templates (
    id              SMALLINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    code            TEXT NOT NULL UNIQUE,   -- 'KHOTMIL_TURN', 'SETORAN_REVIEWED', ...
    title_template  TEXT NOT NULL,          -- 'Giliran baca: Juz {{juz}}'
    body_template   TEXT NOT NULL,
    default_channel notification_channel NOT NULL DEFAULT 'IN_APP',
    is_active       BOOLEAN NOT NULL DEFAULT true
);

-- Satu baris = satu notifikasi utk satu user (fan-out ditulis worker)
CREATE TABLE user_notifications (
    id            BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id       BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    template_code TEXT,                     -- denormalisasi, NULL utk notif ad-hoc
    title         TEXT NOT NULL,
    body          TEXT NOT NULL,
    data          JSONB NOT NULL DEFAULT '{}',  -- deeplink, entity id, dsb.
    channel       notification_channel NOT NULL DEFAULT 'IN_APP',
    sent_at       TIMESTAMPTZ,
    read_at       TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX user_notifications_inbox_idx  ON user_notifications (user_id, created_at DESC);
CREATE INDEX user_notifications_unread_idx ON user_notifications (user_id) WHERE read_at IS NULL;
