-- =============================================================
-- MQ Digital Platform — 0007 Learning (reading progress & materi)
-- Progress MEMBACA dipisah tegas dari progress HAFALAN (0008).
-- =============================================================

-- State "terakhir dibaca" — 1 baris per user, hot path di Home screen
CREATE TABLE user_reading_progress (
    user_id      BIGINT PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    last_ayah_id BIGINT REFERENCES quran_ayahs (id),
    last_page    SMALLINT,
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE bookmarks (
    id         BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id    BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    ayah_id    BIGINT NOT NULL REFERENCES quran_ayahs (id),
    note       TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, ayah_id)
);

CREATE INDEX bookmarks_user_idx ON bookmarks (user_id, created_at DESC);

-- Materi pembelajaran (termasuk halaman materi tajwid per rule —
-- pengganti MVP untuk tajwid warna per-karakter).
CREATE TABLE learning_materials (
    id             BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    slug           TEXT NOT NULL UNIQUE,
    title          TEXT NOT NULL,
    tajwid_rule_id SMALLINT REFERENCES tajwid_rules (id),  -- NULL = materi umum
    content_md     TEXT NOT NULL,           -- markdown
    cover_media_id BIGINT REFERENCES media (id),
    status         material_status NOT NULL DEFAULT 'DRAFT',
    published_at   TIMESTAMPTZ,
    created_by     BIGINT REFERENCES users (id),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX learning_materials_published_idx ON learning_materials (published_at DESC)
    WHERE status = 'PUBLISHED';
