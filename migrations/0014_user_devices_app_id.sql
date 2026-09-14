-- =============================================================
-- MQ Digital Platform — 0014 (MySQL) user_devices.app_id
-- (Bagian III M1.3 — push routing SANTRI_APP/USTADZ_APP/ADMIN_WEB)
-- =============================================================

ALTER TABLE user_devices
    ADD COLUMN app_id VARCHAR(20) NOT NULL DEFAULT 'SANTRI_APP'
        CHECK (app_id IN ('SANTRI_APP', 'USTADZ_APP', 'ADMIN_WEB'))
        AFTER platform;

CREATE INDEX user_devices_user_app_idx ON user_devices (user_id, app_id);
