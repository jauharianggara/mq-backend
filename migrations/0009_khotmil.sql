-- =============================================================
-- MQ Digital Platform — 0009 Khotmil Qur'an Online
--
-- Definisi operasional "JUZ SELESAI" (anti-gaming klaim khataman):
--   1. pages_read  >= 20   (1 juz ~ 20-22 halaman mushaf), DAN
--   2. minutes_read >= campaign.min_minutes_per_juz (default 30 menit), DAN
--   3. verification = AUTO_VERIFIED (kalau campaign tidak minta verifikasi
--      manual) atau VERIFIED oleh ustadz/pengurus.
-- Hanya juz dengan status COMPLETED + terverifikasi yang dihitung
-- menuju khataman. Rate-limit update progress ditegakkan di service
-- (Redis-based), bukan di DB.
--
-- Integritas pembagian juz: partial UNIQUE INDEX pada (campaign_id, juz)
-- untuk assignment AKTIF = jaminan level database bahwa satu juz hanya
-- dipegang satu orang aktif per campaign, di luar transaksi aplikasi.
-- =============================================================

CREATE TABLE khatmil_campaigns (
    id                          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    slug                        TEXT NOT NULL UNIQUE,
    name                        TEXT NOT NULL,       -- "Khataman Qur'an Nasional #001"
    description                 TEXT,
    mode                        campaign_mode   NOT NULL DEFAULT 'PARALLEL',
    status                      campaign_status NOT NULL DEFAULT 'DRAFT',
    target_khataman             INTEGER NOT NULL DEFAULT 1,   -- 0 = tanpa target
    period_start                DATE,
    period_end                  DATE,
    min_minutes_per_juz         SMALLINT NOT NULL DEFAULT 30 CHECK (min_minutes_per_juz BETWEEN 1 AND 600),
    require_manual_verification BOOLEAN NOT NULL DEFAULT false,
    max_participants            INTEGER,
    created_by                  BIGINT NOT NULL REFERENCES users (id),
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT khatmil_period_chk
        CHECK (period_end IS NULL OR period_start IS NULL OR period_end >= period_start)
);

CREATE TABLE khatmil_participants (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    campaign_id BIGINT NOT NULL REFERENCES khatmil_campaigns (id) ON DELETE CASCADE,
    user_id     BIGINT NOT NULL REFERENCES users (id),
    joined_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (campaign_id, user_id)
);

CREATE INDEX khatmil_participants_user_idx ON khatmil_participants (user_id);

CREATE TABLE khatmil_juz_assignments (
    id             BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    campaign_id    BIGINT NOT NULL REFERENCES khatmil_campaigns (id) ON DELETE CASCADE,
    participant_id BIGINT NOT NULL REFERENCES khatmil_participants (id) ON DELETE CASCADE,
    juz            SMALLINT NOT NULL CHECK (juz BETWEEN 1 AND 30),
    status         juz_status NOT NULL DEFAULT 'ASSIGNED',
    assigned_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    due_at         TIMESTAMPTZ               -- dipakai mode SEQUENTIAL (giliran)
);

-- Jaminan DB: satu juz hanya boleh punya SATU pemegang aktif per campaign.
-- Reassignment: tandai lama EXPIRED/REASSIGNED dulu, baru insert baru.
CREATE UNIQUE INDEX khatmil_juz_active_uq
    ON khatmil_juz_assignments (campaign_id, juz)
    WHERE status IN ('ASSIGNED', 'IN_PROGRESS');

CREATE INDEX khatmil_juz_participant_idx ON khatmil_juz_assignments (participant_id);

-- Rollup progress per assignment (diperbarui bersamaan dengan insert event,
-- dalam satu transaksi service)
CREATE TABLE khatmil_progress (
    assignment_id BIGINT PRIMARY KEY REFERENCES khatmil_juz_assignments (id) ON DELETE CASCADE,
    pages_read    SMALLINT NOT NULL DEFAULT 0 CHECK (pages_read  BETWEEN 0 AND 22),
    minutes_read  SMALLINT NOT NULL DEFAULT 0 CHECK (minutes_read BETWEEN 0 AND 1440),
    verification  verification_status NOT NULL DEFAULT 'PENDING',
    verified_by   BIGINT REFERENCES users (id),
    verified_at   TIMESTAMPTZ,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Event log append-only: sumber kebenaran audit progress
CREATE TABLE khatmil_progress_events (
    id            BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    assignment_id BIGINT NOT NULL REFERENCES khatmil_juz_assignments (id) ON DELETE CASCADE,
    user_id       BIGINT NOT NULL REFERENCES users (id),
    pages_read    SMALLINT NOT NULL CHECK (pages_read  > 0),
    minutes_read  SMALLINT NOT NULL CHECK (minutes_read > 0),
    note          TEXT,
    recorded_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX khatmil_progress_events_assign_idx ON khatmil_progress_events (assignment_id, recorded_at);

-- Satu khataman = 30 juz campaign tuntas & terverifikasi (siklus campaign)
CREATE TABLE khatmil_completions (
    id           BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    campaign_id  BIGINT NOT NULL REFERENCES khatmil_campaigns (id) ON DELETE CASCADE,
    cycle        SMALLINT NOT NULL CHECK (cycle >= 1),
    completed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (campaign_id, cycle)
);
