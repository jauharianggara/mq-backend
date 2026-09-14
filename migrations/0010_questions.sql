-- =============================================================
-- MQ Digital Platform — 0010 (MySQL) Tanya Ustadz
--
-- Keputusan desain:
-- 1. Anonimitas: questions.is_anonymous — jawaban & tampilan publik
--    menyembunyikan identitas penanya; ustadz/moderator tetap tahu
--    user_id di backend (kebutuhan anti-penyalahgunaan).
-- 2. Assignment otomatis: rotasi least-load per kategori (ustadz dengan
--    pertanyaan aktif tersedikit & sesuai ustadz_specializations).
--    Penugasan manual MODERATOR = fallback.
-- 3. Moderation state machine (v11): ANSWERED -> PUBLISH_REQUESTED ->
--    PUBLISHED | REJECTED; approve/reject oleh MODERATOR/admin via API
--    admin (audit trail di question_status_history).
-- 4. Search: MySQL FULLTEXT (title, body) + fallback LIKE.
--    (DEGRADASI dari PG tsvector+pg_trgm: typo-tolerant search hilang;
--     meilisearch/tantivy = backlog. Kolom generated search_tsv PG tidak
--     dipindah.)
-- =============================================================

CREATE TABLE question_categories (
    id         SMALLINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    slug       VARCHAR(100) NOT NULL UNIQUE,  -- 'fiqh','ngaji','tajwid','akhlaq','keluarga','muamalah'
    name       VARCHAR(150) NOT NULL,
    icon       VARCHAR(50),
    sort_order SMALLINT NOT NULL DEFAULT 0,
    is_active  TINYINT(1) NOT NULL DEFAULT 1
) ENGINE=InnoDB;

CREATE TABLE ustadz_specializations (
    ustadz_id   BIGINT   NOT NULL,
    category_id SMALLINT NOT NULL,
    PRIMARY KEY (ustadz_id, category_id),
    CONSTRAINT us_ustadz_fk   FOREIGN KEY (ustadz_id) REFERENCES ustadz_profiles (user_id) ON DELETE CASCADE,
    CONSTRAINT us_category_fk FOREIGN KEY (category_id) REFERENCES question_categories (id)
) ENGINE=InnoDB;

CREATE TABLE questions (
    id                   BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    user_id              BIGINT NOT NULL,
    category_id          SMALLINT NOT NULL,
    is_anonymous         TINYINT(1) NOT NULL DEFAULT 0,
    title                VARCHAR(500) NOT NULL,
    body                 TEXT,
    status               VARCHAR(50) NOT NULL DEFAULT 'QUEUED',
    published_message_id BIGINT NULL,               -- FK ditambahkan setelah question_messages
    approved_by          BIGINT NULL,
    published_at         TIMESTAMP NULL,
    closed_at            TIMESTAMP NULL,
    created_at           TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at           TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    CONSTRAINT q_status_chk CHECK (status IN ('PENDING_MODERATION','QUEUED','ASSIGNED','ANSWERED','PUBLISH_REQUESTED','CLOSED','REJECTED','PUBLISHED')),
    CONSTRAINT q_title_chk CHECK (CHAR_LENGTH(title) BETWEEN 5 AND 200),
    CONSTRAINT q_user_fk     FOREIGN KEY (user_id) REFERENCES users (id),
    CONSTRAINT q_category_fk FOREIGN KEY (category_id) REFERENCES question_categories (id),
    CONSTRAINT q_approver_fk FOREIGN KEY (approved_by) REFERENCES users (id),
    KEY questions_queue_idx (status, created_at),
    KEY questions_public_idx (status, published_at DESC),
    FULLTEXT KEY questions_fts_idx (title, body)
) ENGINE=InnoDB;

CREATE TABLE question_messages (
    id          BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    question_id BIGINT NOT NULL,
    sender_id   BIGINT NOT NULL,
    type        VARCHAR(20) NOT NULL DEFAULT 'TEXT',
    content     TEXT,
    media_id    BIGINT NULL,                        -- voice note / gambar / dokumen
    duration_ms INT NULL,
    created_at  TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT qm_type_chk CHECK (type IN ('TEXT','VOICE','IMAGE','FILE')),
    CONSTRAINT qm_question_fk FOREIGN KEY (question_id) REFERENCES questions (id) ON DELETE CASCADE,
    CONSTRAINT qm_sender_fk   FOREIGN KEY (sender_id) REFERENCES users (id),
    CONSTRAINT qm_media_fk    FOREIGN KEY (media_id) REFERENCES media (id),
    KEY question_messages_question_idx (question_id, created_at)
) ENGINE=InnoDB;

ALTER TABLE questions
    ADD CONSTRAINT questions_published_message_fk
    FOREIGN KEY (published_message_id) REFERENCES question_messages (id);

CREATE TABLE question_assignments (
    id           BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    question_id  BIGINT NOT NULL,
    ustadz_id    BIGINT NOT NULL,
    status       VARCHAR(30) NOT NULL DEFAULT 'ASSIGNED',
    assigned_at  TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    responded_at TIMESTAMP NULL,
    CONSTRAINT qa_status_chk CHECK (status IN ('ASSIGNED','ACCEPTED','DECLINED','REASSIGNED')),
    CONSTRAINT qa_question_fk FOREIGN KEY (question_id) REFERENCES questions (id) ON DELETE CASCADE,
    CONSTRAINT qa_ustadz_fk  FOREIGN KEY (ustadz_id) REFERENCES users (id),
    -- Dipakai hitung least-load saat rotasi assignment
    KEY question_assignments_open_idx (status, ustadz_id),
    KEY question_assignments_question_idx (question_id)
) ENGINE=InnoDB;

CREATE TABLE question_status_history (
    id          BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    question_id BIGINT NOT NULL,
    from_status VARCHAR(50) NULL,
    to_status   VARCHAR(50) NOT NULL,
    changed_by  BIGINT NULL,                        -- NULL = system/auto
    note        TEXT,
    created_at  TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT qsh_to_chk CHECK (to_status IN ('PENDING_MODERATION','QUEUED','ASSIGNED','ANSWERED','PUBLISH_REQUESTED','CLOSED','REJECTED','PUBLISHED')),
    CONSTRAINT qsh_from_chk CHECK (from_status IS NULL OR from_status IN ('PENDING_MODERATION','QUEUED','ASSIGNED','ANSWERED','PUBLISH_REQUESTED','CLOSED','REJECTED','PUBLISHED')),
    CONSTRAINT qsh_question_fk FOREIGN KEY (question_id) REFERENCES questions (id) ON DELETE CASCADE,
    CONSTRAINT qsh_changer_fk FOREIGN KEY (changed_by) REFERENCES users (id),
    KEY question_status_history_q_idx (question_id, created_at)
) ENGINE=InnoDB;

-- Metrik SLA (avg response time) dihitung dari timestamp di tabel ini.
