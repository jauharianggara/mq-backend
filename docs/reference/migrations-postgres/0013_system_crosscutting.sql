-- =============================================================
-- MQ Digital Platform — 0013 Cross-cutting: audit, activity, settings, jobs
--
-- Peran infrastruktur yang disepakati:
--   Redis    : cache (list surah, counter progress campaign), rate-limit
--              (login/OTP/update progress khotmil), presence ustadz.
--              BUKAN message queue di MVP.
--   Job queue: Postgres-backed lewat tabel scheduled_jobs di bawah
--              (durable, gampang diobservasi). Worker = loop sqlx / apalis.
--              Redis queue dipertimbangkan lagi kalau throughput naik.
--   Storage  : Cloudflare R2 / MinIO (egress murah utk audio hafalan).
--   Search   : FTS + pg_trgm (lihat 0010). Tanpa Elasticsearch.
-- =============================================================

CREATE TABLE audit_logs (
    id         BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    actor_id   BIGINT REFERENCES users (id),    -- NULL = aksi sistem
    action     TEXT NOT NULL,                   -- 'CREATE' | 'UPDATE' | 'DELETE' | ...
    module     TEXT NOT NULL,                   -- 'ustadz', 'khotmil', 'questions', ...
    entity_type TEXT NOT NULL,                  -- 'ustadz_profiles', ...
    entity_id  TEXT,                            -- TEXT agar fleksibel utk semua tabel
    old_value  JSONB,
    new_value  JSONB,
    ip_address INET,
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX audit_entity_idx ON audit_logs (entity_type, entity_id, created_at DESC);
CREATE INDEX audit_actor_idx  ON audit_logs (actor_id, created_at DESC);

-- Satu event table yang ditulis SEMUA modul — fondasi Learning Journey /
-- Home personal + reporting tanpa scattered logging.
CREATE TABLE activity_events (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id     BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    event_type  TEXT NOT NULL,   -- 'reading.session', 'memorization.submitted',
                                 -- 'khotmil.joined', 'question.asked', 'material.read', ...
    ref_type    TEXT,
    ref_id      TEXT,
    payload     JSONB NOT NULL DEFAULT '{}',
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX activity_user_time_idx ON activity_events (user_id, occurred_at DESC);
CREATE INDEX activity_type_time_idx ON activity_events (event_type, occurred_at DESC);
-- Catatan kapasitas: siapkan partisi bulanan saat volume mendekati ~50 juta baris.

CREATE TABLE settings (
    key         TEXT PRIMARY KEY,            -- 'registration_enabled', 'voice_note_max_mb', ...
    value       JSONB NOT NULL,
    description TEXT,
    updated_by  BIGINT REFERENCES users (id),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Queue durabel utk pekerjaan asinkron: reminder giliran khotmil,
-- notifikasi setoran diperiksa, cek khataman campaign, fan-out broadcast.
CREATE TABLE scheduled_jobs (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    job_type    TEXT NOT NULL,   -- 'khotmil.turn_notify', 'campaign.khatam_check',
                                 -- 'notif.push_deliver', 'media.scan_virus', ...
    payload     JSONB NOT NULL DEFAULT '{}',
    status      job_status NOT NULL DEFAULT 'PENDING',
    run_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at  TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    attempts    SMALLINT NOT NULL DEFAULT 0,
    last_error  TEXT
);

-- Poll query worker: FOR UPDATE SKIP LOCKED pada index ini
CREATE INDEX scheduled_jobs_poll_idx ON scheduled_jobs (run_at) WHERE status = 'PENDING';
