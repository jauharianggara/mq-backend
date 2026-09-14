-- =============================================================
-- MQ Digital Platform — 0013 (MySQL) Cross-cutting: audit, activity, settings, jobs
--
-- Peran infrastruktur (final, v6/v7):
--   Cache/rate-limit : IN-PROCESS (moka + governor) via adapter CacheStore;
--                      Redis = optional feature saat VPS multi-instance.
--   Job queue        : tabel scheduled_jobs di bawah (durable, mudah
--                      diobservasi). Worker = loop sqlx FOR UPDATE SKIP LOCKED
--                      (MySQL 8) — tandai RUNNING di transaksi pendek,
--                      eksekusi DI LUAR lock; dedupe & stale-recovery v11.
--   Storage          : SeaweedFS lokal (S3 API) / R2 offsite.
--   Search           : FULLTEXT + LIKE (lihat 0010). Tanpa Elasticsearch.
-- =============================================================

CREATE TABLE audit_logs (
    id          BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    actor_id    BIGINT NULL,                        -- NULL = aksi sistem
    action      VARCHAR(50) NOT NULL,               -- 'CREATE' | 'UPDATE' | 'DELETE' | ...
    module      VARCHAR(50) NOT NULL,               -- 'ustadz', 'khatmil', 'questions', ...
    entity_type VARCHAR(100) NOT NULL,              -- 'ustadz_profiles', ...
    entity_id   VARCHAR(64),                        -- TEXT-terbatah agar fleksibel utk semua tabel
    old_value   JSON NULL,
    new_value   JSON NULL,
    ip_address  VARCHAR(45),                        -- asal INET
    user_agent  TEXT,
    created_at  TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT al_actor_fk FOREIGN KEY (actor_id) REFERENCES users (id),
    KEY audit_entity_idx (entity_type, entity_id, created_at DESC),
    KEY audit_actor_idx (actor_id, created_at DESC)
) ENGINE=InnoDB;

-- Satu event table yang ditulis SEMUA modul — fondasi Learning Journey /
-- Home personal + reporting tanpa scattered logging.
CREATE TABLE activity_events (
    id          BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    user_id     BIGINT NOT NULL,
    event_type  VARCHAR(100) NOT NULL,  -- 'reading.session', 'memorization.submitted',
                                        -- 'khatmil.joined', 'question.asked', 'material.read', ...
    ref_type    VARCHAR(50),
    ref_id      VARCHAR(64),
    payload     JSON NOT NULL,
    occurred_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT ae_user_fk FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    KEY activity_user_time_idx (user_id, occurred_at DESC),
    KEY activity_type_time_idx (event_type, occurred_at DESC)
) ENGINE=InnoDB;
-- Catatan kapasitas: siapkan partisi bulanan saat volume mendekati ~50 juta baris.

CREATE TABLE settings (
    `key`       VARCHAR(150) NOT NULL PRIMARY KEY,  -- 'registration_enabled', 'voice_note_max_mb', ...
    value       JSON NOT NULL,
    description TEXT,
    updated_by  BIGINT NULL,
    updated_at  TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    CONSTRAINT st_updater_fk FOREIGN KEY (updated_by) REFERENCES users (id)
) ENGINE=InnoDB;

-- Queue durabel utk pekerjaan asinkron: reminder giliran khatmil,
-- notifikasi setoran diperiksa, cek khataman campaign, fan-out broadcast,
-- media orphan cleanup (v11).
-- v11 (worker reliability): dedupe_key unique utk job ber-side-effect;
-- locked_at utk recovery stale RUNNING; retry+backoff di worker.
CREATE TABLE scheduled_jobs (
    id          BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    job_type    VARCHAR(100) NOT NULL,  -- 'khatmil.turn_notify', 'campaign.khatam_check',
                                        -- 'notif.push_deliver', 'media.cleanup_orphan', ...
    payload     JSON NOT NULL,
    status      VARCHAR(20) NOT NULL DEFAULT 'PENDING',
    run_at      TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    started_at  TIMESTAMP NULL,
    finished_at TIMESTAMP NULL,
    attempts    SMALLINT NOT NULL DEFAULT 0,
    last_error  TEXT,
    dedupe_key  VARCHAR(191) NULL,      -- unique utk job ber-side-effect (NULL = bebas)
    locked_at   TIMESTAMP NULL,         -- stale-RUNNING recovery
    CONSTRAINT sj_status_chk CHECK (status IN ('PENDING','RUNNING','DONE','FAILED','DEAD')),
    UNIQUE KEY sj_dedupe_uq (dedupe_key),
    -- Poll query worker: FOR UPDATE SKIP LOCKED pada index ini
    KEY scheduled_jobs_poll_idx (status, run_at)
) ENGINE=InnoDB;
