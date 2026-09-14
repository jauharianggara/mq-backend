-- =============================================================
-- MQ Digital Platform — 0004 Media (object storage registry)
-- File fisik disimpan di Cloudflare R2 / MinIO; DB hanya metadata.
-- Limit voice note (default: 10 MB, 5 menit) divalidasi di service
-- + bisa diubah via settings, jadi tidak dikunci di CHECK constraint.
-- =============================================================

CREATE TABLE media (
    id              BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    owner_id        BIGINT REFERENCES users (id),   -- NULL = media sistem
    kind            media_kind   NOT NULL,
    status          media_status NOT NULL DEFAULT 'UPLOADING',
    bucket          TEXT         NOT NULL,
    storage_key     TEXT         NOT NULL UNIQUE,   -- object key: hafalan/2026/09/xxx.m4a
    mime_type       TEXT         NOT NULL,
    byte_size       BIGINT       NOT NULL CHECK (byte_size > 0),
    duration_ms     INTEGER      CHECK (duration_ms IS NULL OR duration_ms > 0),
    checksum_sha256 TEXT,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT now()
);

CREATE INDEX media_owner_idx ON media (owner_id, created_at DESC);
