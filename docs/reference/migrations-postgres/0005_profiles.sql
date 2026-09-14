-- =============================================================
-- MQ Digital Platform — 0005 Profil Santri & Ustadz
-- =============================================================

CREATE TABLE user_profiles (
    user_id        BIGINT PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    full_name      TEXT NOT NULL,
    gender         gender,
    birth_date     DATE,
    address_text   TEXT,
    city           TEXT,          -- teks sederhana; wilayah leaderboard nyusul Phase 2
    province       TEXT,
    photo_media_id BIGINT REFERENCES media (id),
    bio            TEXT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ----------------------------------------------------------------

CREATE TABLE ustadz_profiles (
    user_id              BIGINT PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    code                 TEXT UNIQUE,          -- nomor induk asatidz
    title                TEXT,                 -- 'Ustadz' / 'Ustadzah'
    bio                  TEXT,
    photo_media_id       BIGINT REFERENCES media (id),
    is_accepting_questions BOOLEAN NOT NULL DEFAULT true,
    max_active_questions   SMALLINT NOT NULL DEFAULT 10 CHECK (max_active_questions BETWEEN 1 AND 100),
    verified_at          TIMESTAMPTZ,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Spesialisasi ustadz didefinisikan di 0010_questions.sql
-- karena mereferensi question_categories.
