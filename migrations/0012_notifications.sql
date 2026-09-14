-- =============================================================
-- MQ Digital Platform — 0012 (MySQL) Notification
-- MVP: channel IN_APP + PUSH (FCM). EMAIL/WHATSAPP disiapkan
-- enum-nya, pengirimnya menyusul. Push token disimpan di
-- user_devices (0003), tidak ada tabel device_tokens terpisah.
-- =============================================================

CREATE TABLE notification_templates (
    id              SMALLINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    code            VARCHAR(100) NOT NULL UNIQUE,   -- 'KHOTMIL_TURN', 'SETORAN_REVIEWED', ...
    title_template  VARCHAR(255) NOT NULL,          -- 'Giliran baca: Juz {{juz}}'
    body_template   TEXT NOT NULL,
    default_channel VARCHAR(30) NOT NULL DEFAULT 'IN_APP',
    is_active       TINYINT(1) NOT NULL DEFAULT 1,
    CONSTRAINT nt_channel_chk CHECK (default_channel IN ('IN_APP','PUSH','EMAIL','WHATSAPP'))
) ENGINE=InnoDB;

-- Satu baris = satu notifikasi utk satu user (fan-out ditulis worker)
CREATE TABLE user_notifications (
    id            BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    user_id       BIGINT NOT NULL,
    template_code VARCHAR(100) NULL,                -- denormalisasi, NULL utk notif ad-hoc
    title         VARCHAR(255) NOT NULL,
    body          TEXT NOT NULL,
    data          JSON NOT NULL,                    -- deeplink, entity id, dsb. (service set '{}')
    channel       VARCHAR(30) NOT NULL DEFAULT 'IN_APP',
    sent_at       TIMESTAMP NULL,
    read_at       TIMESTAMP NULL,
    created_at    TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT un_channel_chk CHECK (channel IN ('IN_APP','PUSH','EMAIL','WHATSAPP')),
    CONSTRAINT un_user_fk FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    KEY user_notifications_inbox_idx (user_id, created_at DESC),
    KEY user_notifications_unread_idx (user_id, read_at)
) ENGINE=InnoDB;
