-- =============================================================
-- MQ Digital Platform — 0003 Auth & RBAC
-- =============================================================

CREATE TABLE users (
    id                 BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    phone              VARCHAR(20),
    email              CITEXT,
    password_hash      TEXT,                          -- nullable bila login OTP/WA saja
    account_type       account_type NOT NULL DEFAULT 'SANTRI',
    status             user_status  NOT NULL DEFAULT 'PENDING_VERIFICATION',
    phone_verified_at  TIMESTAMPTZ,
    email_verified_at  TIMESTAMPTZ,
    last_login_at      TIMESTAMPTZ,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT users_contact_chk CHECK (phone IS NOT NULL OR email IS NOT NULL)
);

CREATE UNIQUE INDEX users_phone_uq ON users (phone)     WHERE phone IS NOT NULL;
CREATE UNIQUE INDEX users_email_uq ON users (email)     WHERE email IS NOT NULL;

-- ----------------------------------------------------------------

CREATE TABLE roles (
    id          SMALLINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    code        TEXT NOT NULL UNIQUE,        -- SUPER_ADMIN, ADMIN, MODERATOR, USTADZ, SANTRI
    name        TEXT NOT NULL,
    description TEXT
);

CREATE TABLE permissions (
    id          INTEGER GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    code        TEXT NOT NULL UNIQUE,        -- contoh: khotmil.manage, questions.answer
    module      TEXT NOT NULL,
    description TEXT
);

CREATE TABLE role_permissions (
    role_id       SMALLINT NOT NULL REFERENCES roles (id)       ON DELETE CASCADE,
    permission_id INTEGER  NOT NULL REFERENCES permissions (id) ON DELETE CASCADE,
    PRIMARY KEY (role_id, permission_id)
);

CREATE TABLE user_roles (
    user_id     BIGINT   NOT NULL REFERENCES users (id)  ON DELETE CASCADE,
    role_id     SMALLINT NOT NULL REFERENCES roles (id)  ON DELETE CASCADE,
    assigned_by BIGINT   REFERENCES users (id),
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, role_id)
);

CREATE INDEX user_roles_role_idx ON user_roles (role_id);

-- ----------------------------------------------------------------

CREATE TABLE user_devices (
    id           BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id      BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    platform     device_platform NOT NULL,
    push_token   TEXT,                          -- FCM token; NULL utk WEB tanpa notif
    device_name  TEXT,
    app_version  TEXT,
    last_seen_at TIMESTAMPTZ,
    is_active    BOOLEAN     NOT NULL DEFAULT true,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX user_devices_token_uq ON user_devices (push_token) WHERE push_token IS NOT NULL;
CREATE INDEX        user_devices_user_idx ON user_devices (user_id);

-- ----------------------------------------------------------------

CREATE TABLE user_sessions (
    id                 BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id            BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    refresh_token_hash TEXT NOT NULL UNIQUE,   -- simpan hash, bukan token mentah
    device_id          BIGINT REFERENCES user_devices (id) ON DELETE SET NULL,
    ip_address         INET,
    user_agent         TEXT,
    expires_at         TIMESTAMPTZ NOT NULL,
    revoked_at         TIMESTAMPTZ,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX user_sessions_active_idx ON user_sessions (user_id) WHERE revoked_at IS NULL;
