-- =============================================================
-- MQ Digital Platform — 0003 (MySQL) Auth & RBAC
-- Konversi dari PG (docs/reference/migrations-postgres/0003):
--   CITEXT->VARCHAR (collation CI), INET->VARCHAR(45), enum->VARCHAR+CHECK,
--   partial unique (nullable)->UNIQUE biasa (MySQL abaikan NULL),
--   TIMESTAMP = UTC (pool wajib time_zone='+00:00').
-- =============================================================

CREATE TABLE users (
    id                 BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    phone              VARCHAR(20),
    email              VARCHAR(255),
    password_hash      TEXT,                          -- nullable bila login OTP/WA saja
    account_type       VARCHAR(50) NOT NULL DEFAULT 'SANTRI',
    status             VARCHAR(50) NOT NULL DEFAULT 'PENDING_VERIFICATION',
    phone_verified_at  TIMESTAMP NULL,
    email_verified_at  TIMESTAMP NULL,
    last_login_at      TIMESTAMP NULL,
    created_at         TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at         TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    CONSTRAINT users_account_type_chk CHECK (account_type IN ('SANTRI','UMUM','PENGURUS','ORANG_TUA','DONATUR')),
    CONSTRAINT users_status_chk       CHECK (status IN ('PENDING_VERIFICATION','ACTIVE','SUSPENDED','DEACTIVATED','DELETED')),
    CONSTRAINT users_contact_chk      CHECK (phone IS NOT NULL OR email IS NOT NULL),
    UNIQUE KEY users_phone_uq (phone),
    UNIQUE KEY users_email_uq (email)
) ENGINE=InnoDB;

CREATE TABLE roles (
    id          SMALLINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    code        VARCHAR(100) NOT NULL UNIQUE,   -- SUPER_ADMIN, ADMIN, MODERATOR, USTADZ, SANTRI
    name        VARCHAR(150) NOT NULL,
    description TEXT
) ENGINE=InnoDB;

CREATE TABLE permissions (
    id          INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    code        VARCHAR(100) NOT NULL UNIQUE,   -- contoh: khatmil.manage, questions.answer
    module      VARCHAR(50) NOT NULL,
    description TEXT
) ENGINE=InnoDB;

CREATE TABLE role_permissions (
    role_id       SMALLINT NOT NULL,
    permission_id INT      NOT NULL,
    PRIMARY KEY (role_id, permission_id),
    CONSTRAINT rp_role_fk FOREIGN KEY (role_id) REFERENCES roles (id) ON DELETE CASCADE,
    CONSTRAINT rp_perm_fk FOREIGN KEY (permission_id) REFERENCES permissions (id) ON DELETE CASCADE
) ENGINE=InnoDB;

CREATE TABLE user_roles (
    user_id     BIGINT   NOT NULL,
    role_id     SMALLINT NOT NULL,
    assigned_by BIGINT   NULL,
    assigned_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, role_id),
    CONSTRAINT ur_user_fk FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    CONSTRAINT ur_role_fk FOREIGN KEY (role_id) REFERENCES roles (id) ON DELETE CASCADE,
    CONSTRAINT ur_assigner_fk FOREIGN KEY (assigned_by) REFERENCES users (id),
    KEY user_roles_role_idx (role_id)
) ENGINE=InnoDB;

CREATE TABLE user_devices (
    id           BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    user_id      BIGINT NOT NULL,
    platform     VARCHAR(50) NOT NULL,
    push_token   VARCHAR(255),                    -- FCM token; NULL utk WEB tanpa notif
    device_name  TEXT,
    app_version  TEXT,
    last_seen_at TIMESTAMP NULL,
    is_active    TINYINT(1) NOT NULL DEFAULT 1,
    created_at   TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at   TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    CONSTRAINT ud_platform_chk CHECK (platform IN ('ANDROID','IOS','WEB')),
    CONSTRAINT ud_user_fk FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    UNIQUE KEY user_devices_token_uq (push_token),
    KEY user_devices_user_idx (user_id)
) ENGINE=InnoDB;

CREATE TABLE user_sessions (
    id                 BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    user_id            BIGINT NOT NULL,
    refresh_token_hash VARCHAR(255) NOT NULL UNIQUE,  -- simpan hash, bukan token mentah
    device_id          BIGINT NULL,
    ip_address         VARCHAR(45),                   -- asal INET (IPv4/IPv6)
    user_agent         TEXT,
    expires_at         TIMESTAMP NOT NULL,
    revoked_at         TIMESTAMP NULL,
    created_at         TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT us_user_fk FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    CONSTRAINT us_device_fk FOREIGN KEY (device_id) REFERENCES user_devices (id) ON DELETE SET NULL,
    KEY user_sessions_active_idx (user_id, revoked_at)
) ENGINE=InnoDB;
