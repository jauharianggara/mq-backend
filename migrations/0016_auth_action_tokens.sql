-- =============================================================
-- MQ Digital Platform — 0016 (MySQL) auth action tokens
-- (Bagian I v11 — verification/reset token: one-time, hashed at rest,
--  expiry, used_at; resend invalidates prior token di service)
-- =============================================================

CREATE TABLE auth_action_tokens (
    id         BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    user_id    BIGINT NOT NULL,
    purpose    VARCHAR(20) NOT NULL,   -- EMAIL_VERIFY | PASSWORD_RESET
    token_hash CHAR(64) NOT NULL UNIQUE,  -- SHA-256 hex; token mentah TIDAK pernah disimpan
    expires_at TIMESTAMP NOT NULL,
    used_at    TIMESTAMP NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT aat_purpose_chk CHECK (purpose IN ('EMAIL_VERIFY', 'PASSWORD_RESET')),
    CONSTRAINT aat_user_fk FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    KEY aat_user_purpose_idx (user_id, purpose)
) ENGINE=InnoDB;
