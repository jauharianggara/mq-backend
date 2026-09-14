-- =============================================================
-- MQ Digital Platform — 0010 Tanya Ustadz
--
-- Keputusan desain:
-- 1. Anonimitas: questions.is_anonymous — jawaban & tampilan publik
--    menyembunyikan identitas penanya; ustadz/moderator tetap tahu
--    user_id di backend (kebutuhan anti-penyalahgunaan).
-- 2. Assignment otomatis: round-robin least-load per kategori
--    (ustadz dengan pertanyaan aktif tersedikit & sesuai
--    ustadz_specializations). Tidak ada moderator manual di MVP.
-- 3. Approval chain sebelum publish ke knowledge base publik:
--    ANSWERED -> CLOSED -> PUBLISHED; kolom approved_by + disclaimer
--    ditampilkan UI pada konten PUBLISHED.
-- 4. Search: PostgreSQL FTS (dictionary 'indonesian') + pg_trgm utk typo.
--    Tidak memakai Elasticsearch.
-- =============================================================

CREATE TABLE question_categories (
    id         SMALLINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    slug       TEXT NOT NULL UNIQUE,      -- 'fiqh', 'ngaji', 'tajwid', 'akhlaq', 'keluarga', 'muamalah'
    name       TEXT NOT NULL,
    icon       TEXT,
    sort_order SMALLINT NOT NULL DEFAULT 0,
    is_active  BOOLEAN NOT NULL DEFAULT true
);

CREATE TABLE ustadz_specializations (
    ustadz_id   BIGINT   NOT NULL REFERENCES ustadz_profiles (user_id) ON DELETE CASCADE,
    category_id SMALLINT NOT NULL REFERENCES question_categories (id),
    PRIMARY KEY (ustadz_id, category_id)
);

CREATE TABLE questions (
    id                   BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id              BIGINT NOT NULL REFERENCES users (id),
    category_id          SMALLINT NOT NULL REFERENCES question_categories (id),
    is_anonymous         BOOLEAN NOT NULL DEFAULT false,
    title                TEXT NOT NULL CHECK (length(title) BETWEEN 5 AND 200),
    body                 TEXT,
    status               question_status NOT NULL DEFAULT 'QUEUED',
    published_message_id BIGINT,          -- FK ditambahkan di bawah (urutan tabel)
    approved_by          BIGINT REFERENCES users (id),
    published_at         TIMESTAMPTZ,
    closed_at            TIMESTAMPTZ,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    search_tsv           tsvector GENERATED ALWAYS AS (
                           to_tsvector('indonesian', coalesce(title, '') || ' ' || coalesce(body, ''))
                         ) STORED
);

CREATE INDEX questions_fts_idx   ON questions USING GIN (search_tsv);
CREATE INDEX questions_trgm_idx  ON questions USING GIN (title gin_trgm_ops);
CREATE INDEX questions_queue_idx ON questions (status, created_at);
CREATE INDEX questions_public_idx ON questions (published_at DESC) WHERE status = 'PUBLISHED';

CREATE TABLE question_messages (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    question_id BIGINT NOT NULL REFERENCES questions (id) ON DELETE CASCADE,
    sender_id   BIGINT NOT NULL REFERENCES users (id),
    type        message_type NOT NULL DEFAULT 'TEXT',
    content     TEXT,
    media_id    BIGINT REFERENCES media (id),       -- voice note / gambar / dokumen
    duration_ms INTEGER,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX question_messages_question_idx ON question_messages (question_id, created_at);

ALTER TABLE questions
    ADD CONSTRAINT questions_published_message_fk
    FOREIGN KEY (published_message_id) REFERENCES question_messages (id);

CREATE TABLE question_assignments (
    id           BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    question_id  BIGINT NOT NULL REFERENCES questions (id) ON DELETE CASCADE,
    ustadz_id    BIGINT NOT NULL REFERENCES users (id),
    status       ustadz_assignment_status NOT NULL DEFAULT 'ASSIGNED',
    assigned_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    responded_at TIMESTAMPTZ
);

-- Dipakai hitung least-load saat rotasi assignment
CREATE INDEX question_assignments_open_idx ON question_assignments (ustadz_id)
    WHERE status = 'ASSIGNED';
CREATE INDEX question_assignments_question_idx ON question_assignments (question_id);

CREATE TABLE question_status_history (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    question_id BIGINT NOT NULL REFERENCES questions (id) ON DELETE CASCADE,
    from_status question_status,
    to_status   question_status NOT NULL,
    changed_by  BIGINT REFERENCES users (id),       -- NULL = system/auto
    note        TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Metrik SLA (avg response time) dihitung dari timestamp di tabel ini,
-- tidak perlu kolom khusus.
CREATE INDEX question_status_history_q_idx ON question_status_history (question_id, created_at);
