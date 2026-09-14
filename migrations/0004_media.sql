-- =============================================================
-- MQ Digital Platform — 0004 (MySQL) Media (object storage registry)
-- File fisik: SeaweedFS lokal (S3 API) / R2; DB hanya metadata.
-- Limit voice note (default: 10 MB, 5 menit) divalidasi di service
-- + bisa diubah via settings (tidak dikunci di CHECK).
-- =============================================================

CREATE TABLE media (
    id              BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    owner_id        BIGINT NULL,                     -- NULL = media sistem
    kind            VARCHAR(50)  NOT NULL,
    status          VARCHAR(50)  NOT NULL DEFAULT 'UPLOADING',
    bucket          VARCHAR(150) NOT NULL,
    storage_key     VARCHAR(255) NOT NULL UNIQUE,    -- object key: hafalan/2026/09/xxx.m4a
    mime_type       VARCHAR(150) NOT NULL,
    byte_size       BIGINT       NOT NULL,
    duration_ms     INT          NULL,
    checksum_sha256 CHAR(64)     NULL,
    created_at      TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT media_kind_chk   CHECK (kind IN ('AUDIO','IMAGE','VIDEO','DOCUMENT')),
    CONSTRAINT media_status_chk CHECK (status IN ('UPLOADING','READY','FAILED')),
    CONSTRAINT media_size_chk   CHECK (byte_size > 0),
    CONSTRAINT media_duration_chk CHECK (duration_ms IS NULL OR duration_ms > 0),
    CONSTRAINT media_owner_fk FOREIGN KEY (owner_id) REFERENCES users (id),
    KEY media_owner_idx (owner_id, created_at DESC)
) ENGINE=InnoDB;
