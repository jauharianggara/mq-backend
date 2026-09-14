-- =============================================================
-- MQ Digital Platform — 0008 Hafalan / Setoran (MVP minimal)
-- Alur MVP: santri submit audio -> masuk antrean ustadz -> review
-- (PASSED / REVISION / REJECTED). Multi-reviewer + SLA + re-review
-- bertingkat = Phase 2 (cukup tambah tabel/kolom, tidak rewrite).
-- =============================================================

CREATE TABLE memorization_submissions (
    id             BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id        BIGINT NOT NULL REFERENCES users (id),
    surah_id       SMALLINT NOT NULL REFERENCES quran_surahs (id),
    ayah_start     SMALLINT NOT NULL CHECK (ayah_start >= 1),
    ayah_end       SMALLINT NOT NULL CHECK (ayah_end >= ayah_start),
    audio_media_id BIGINT NOT NULL REFERENCES media (id),
    duration_ms    INTEGER,
    note           TEXT,                     -- catatan santri (opsional)
    status         submission_status NOT NULL DEFAULT 'PENDING',
    ustadz_id      BIGINT REFERENCES users (id),   -- diisi saat ambil/assign review
    submitted_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    reviewed_at    TIMESTAMPTZ
);

-- Antrean review ustadz: PENDING tanpa ustadz, atau IN_REVIEW milik ustadz
CREATE INDEX memorization_queue_idx ON memorization_submissions (ustadz_id, status, submitted_at)
    WHERE status IN ('PENDING', 'IN_REVIEW');
CREATE INDEX memorization_user_idx ON memorization_submissions (user_id, submitted_at DESC);

CREATE TABLE memorization_reviews (
    id                   BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    submission_id        BIGINT NOT NULL UNIQUE REFERENCES memorization_submissions (id) ON DELETE CASCADE,
    reviewer_id          BIGINT NOT NULL REFERENCES users (id),
    verdict              review_verdict NOT NULL,
    notes                TEXT,
    reply_audio_media_id BIGINT REFERENCES media (id),  -- feedback berupa voice note
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Rollup progress hafalan per surah (ditulis service saat verdict PASSED)
CREATE TABLE memorization_progress (
    user_id          BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    surah_id         SMALLINT NOT NULL REFERENCES quran_surahs (id),
    last_passed_ayah SMALLINT NOT NULL DEFAULT 0,
    passed_count     SMALLINT NOT NULL DEFAULT 0,
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, surah_id)
);
