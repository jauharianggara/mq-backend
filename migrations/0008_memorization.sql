-- =============================================================
-- MQ Digital Platform — 0008 (MySQL) Hafalan / Setoran (MVP minimal)
-- Alur MVP: santri submit audio -> antrean ustadz -> review
-- (PASSED / REVISION / REJECTED). Multi-reviewer + SLA + re-review
-- bertingkat = Phase 2.
-- Kolom client_key (0015) menyusul via migration M1.3 Bagian III.
-- =============================================================

CREATE TABLE memorization_submissions (
    id             BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    user_id        BIGINT NOT NULL,
    surah_id       SMALLINT NOT NULL,
    ayah_start     SMALLINT NOT NULL,
    ayah_end       SMALLINT NOT NULL,
    audio_media_id BIGINT NOT NULL,
    duration_ms    INT NULL,
    note           TEXT,                             -- catatan santri (opsional)
    status         VARCHAR(50) NOT NULL DEFAULT 'PENDING',
    ustadz_id      BIGINT NULL,                      -- diisi saat ambil/assign review
    submitted_at   TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    reviewed_at    TIMESTAMP NULL,
    CONSTRAINT ms_status_chk CHECK (status IN ('PENDING','IN_REVIEW','PASSED','REVISION','REJECTED')),
    CONSTRAINT ms_range_chk CHECK (ayah_start >= 1 AND ayah_end >= ayah_start),
    CONSTRAINT ms_user_fk   FOREIGN KEY (user_id) REFERENCES users (id),
    CONSTRAINT ms_surah_fk  FOREIGN KEY (surah_id) REFERENCES quran_surahs (id),
    CONSTRAINT ms_media_fk  FOREIGN KEY (audio_media_id) REFERENCES media (id),
    CONSTRAINT ms_ustadz_fk FOREIGN KEY (ustadz_id) REFERENCES users (id),
    -- Antrean review: PENDING tanpa ustadz, atau IN_REVIEW milik ustadz
    KEY memorization_queue_idx (status, ustadz_id, submitted_at),
    KEY memorization_user_idx (user_id, submitted_at DESC)
) ENGINE=InnoDB;

CREATE TABLE memorization_reviews (
    id                   BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    submission_id        BIGINT NOT NULL UNIQUE,
    reviewer_id          BIGINT NOT NULL,
    verdict              VARCHAR(50) NOT NULL,
    notes                TEXT,
    reply_audio_media_id BIGINT NULL,                -- feedback berupa voice note
    created_at           TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT mr_verdict_chk CHECK (verdict IN ('PASSED','REVISION','REJECTED')),
    CONSTRAINT mr_submission_fk FOREIGN KEY (submission_id) REFERENCES memorization_submissions (id) ON DELETE CASCADE,
    CONSTRAINT mr_reviewer_fk   FOREIGN KEY (reviewer_id) REFERENCES users (id),
    CONSTRAINT mr_reply_fk      FOREIGN KEY (reply_audio_media_id) REFERENCES media (id)
) ENGINE=InnoDB;

-- Rollup progress hafalan per surah (ditulis service saat verdict PASSED)
CREATE TABLE memorization_progress (
    user_id          BIGINT NOT NULL,
    surah_id         SMALLINT NOT NULL,
    last_passed_ayah SMALLINT NOT NULL DEFAULT 0,
    passed_count     SMALLINT NOT NULL DEFAULT 0,
    updated_at       TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, surah_id),
    CONSTRAINT mp_user_fk  FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    CONSTRAINT mp_surah_fk FOREIGN KEY (surah_id) REFERENCES quran_surahs (id)
) ENGINE=InnoDB;
