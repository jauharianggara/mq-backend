-- =============================================================
-- MQ Digital Platform — 0009 (MySQL) Khatmil Qur'an Online
--
-- Definisi operasional "JUZ SELESAI" (anti-gaming klaim khataman):
--   1. pages_read  >= 20   (1 juz ~ 20-22 halaman mushaf), DAN
--   2. minutes_read >= campaign.min_minutes_per_juz (default 30 menit), DAN
--   3. verification: SELF_REPORTED / SYSTEM_VERIFIED (memenuhi rule otomatis,
--      BUKAN bukti objektif) / MANUAL_VERIFIED oleh ustadz/pengurus.
--   Hanya juz COMPLETED + terverifikasi yang dihitung menuju khataman.
--   Rate-limit update progress ditegakkan di service (in-process), bukan DB.
--
-- Integritas pembagian juz: partial unique PG (campaign_id, juz) WHERE aktif
--   -> MySQL: kolom generated `active_marker` (1=ASSIGNED/IN_PROGRESS,
--   NULL=sudah lepas) + UNIQUE multi-kolom (MySQL mengabaikan NULL).
--   => SATU pemegang aktif per juz per campaign, dijamin level database.
-- =============================================================

CREATE TABLE khatmil_campaigns (
    id                          BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    slug                        VARCHAR(200) NOT NULL UNIQUE,
    name                        VARCHAR(255) NOT NULL,   -- "Khataman Qur'an Nasional #001"
    description                 TEXT,
    mode                        VARCHAR(20)  NOT NULL DEFAULT 'PARALLEL',
    status                      VARCHAR(30)  NOT NULL DEFAULT 'DRAFT',
    target_khataman             INT NOT NULL DEFAULT 1,  -- 0 = tanpa target
    period_start                DATE NULL,
    period_end                  DATE NULL,
    min_minutes_per_juz         SMALLINT NOT NULL DEFAULT 30,
    require_manual_verification TINYINT(1) NOT NULL DEFAULT 0,
    max_participants            INT NULL,
    created_by                  BIGINT NOT NULL,
    created_at                  TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at                  TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    CONSTRAINT kc_mode_chk   CHECK (mode IN ('PARALLEL','SEQUENTIAL')),
    CONSTRAINT kc_status_chk CHECK (status IN ('DRAFT','SCHEDULED','ACTIVE','COMPLETED','CANCELLED')),
    CONSTRAINT kc_minutes_chk CHECK (min_minutes_per_juz BETWEEN 1 AND 600),
    CONSTRAINT kc_period_chk  CHECK (period_end IS NULL OR period_start IS NULL OR period_end >= period_start),
    CONSTRAINT kc_creator_fk FOREIGN KEY (created_by) REFERENCES users (id)
) ENGINE=InnoDB;

CREATE TABLE khatmil_participants (
    id          BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    campaign_id BIGINT NOT NULL,
    user_id     BIGINT NOT NULL,
    joined_at   TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT kp_campaign_fk FOREIGN KEY (campaign_id) REFERENCES khatmil_campaigns (id) ON DELETE CASCADE,
    CONSTRAINT kp_user_fk     FOREIGN KEY (user_id) REFERENCES users (id),
    UNIQUE KEY kp_campaign_user_uq (campaign_id, user_id),
    KEY khatmil_participants_user_idx (user_id)
) ENGINE=InnoDB;

CREATE TABLE khatmil_juz_assignments (
    id             BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    campaign_id    BIGINT NOT NULL,
    participant_id BIGINT NOT NULL,
    juz            SMALLINT NOT NULL,
    status         VARCHAR(30) NOT NULL DEFAULT 'ASSIGNED',
    assigned_at    TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    due_at         TIMESTAMP NULL,                    -- dipakai mode SEQUENTIAL (giliran)
    -- Marker unik-aktif (turunan status; menggantikan partial unique PG):
    active_marker  TINYINT GENERATED ALWAYS AS (
                       CASE WHEN status IN ('ASSIGNED','IN_PROGRESS') THEN 1 ELSE NULL END
                   ) STORED,
    CONSTRAINT ka_juz_chk    CHECK (juz BETWEEN 1 AND 30),
    CONSTRAINT ka_status_chk CHECK (status IN ('ASSIGNED','IN_PROGRESS','COMPLETED','EXPIRED','REASSIGNED')),
    CONSTRAINT ka_campaign_fk    FOREIGN KEY (campaign_id) REFERENCES khatmil_campaigns (id) ON DELETE CASCADE,
    CONSTRAINT ka_participant_fk FOREIGN KEY (participant_id) REFERENCES khatmil_participants (id) ON DELETE CASCADE,
    -- SATU pemegang aktif per juz per campaign (NULL dilewati unique MySQL):
    UNIQUE KEY khatmil_juz_active_uq (campaign_id, juz, active_marker),
    KEY khatmil_juz_participant_idx (participant_id)
) ENGINE=InnoDB;

-- Rollup progress per assignment (diperbarui bersamaan dengan insert event,
-- dalam satu transaksi service)
CREATE TABLE khatmil_progress (
    assignment_id BIGINT NOT NULL PRIMARY KEY,
    pages_read    SMALLINT NOT NULL DEFAULT 0,
    minutes_read  SMALLINT NOT NULL DEFAULT 0,
    verification  VARCHAR(30) NOT NULL DEFAULT 'PENDING',
    verified_by   BIGINT NULL,
    verified_at   TIMESTAMP NULL,
    updated_at    TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    CONSTRAINT kg_pages_chk    CHECK (pages_read  BETWEEN 0 AND 22),
    CONSTRAINT kg_minutes_chk  CHECK (minutes_read BETWEEN 0 AND 1440),
    CONSTRAINT kg_verify_chk   CHECK (verification IN ('PENDING','SELF_REPORTED','AUTO_VERIFIED','SYSTEM_VERIFIED','VERIFIED','REJECTED')),
    CONSTRAINT kg_assignment_fk FOREIGN KEY (assignment_id) REFERENCES khatmil_juz_assignments (id) ON DELETE CASCADE,
    CONSTRAINT kg_verifier_fk   FOREIGN KEY (verified_by) REFERENCES users (id)
) ENGINE=InnoDB;

-- Event log append-only: sumber kebenaran audit progress
CREATE TABLE khatmil_progress_events (
    id            BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    assignment_id BIGINT NOT NULL,
    user_id       BIGINT NOT NULL,
    pages_read    SMALLINT NOT NULL,
    minutes_read  SMALLINT NOT NULL,
    note          TEXT,
    recorded_at   TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT kge_pages_chk   CHECK (pages_read  > 0),
    CONSTRAINT kge_minutes_chk CHECK (minutes_read > 0),
    CONSTRAINT kge_assignment_fk FOREIGN KEY (assignment_id) REFERENCES khatmil_juz_assignments (id) ON DELETE CASCADE,
    CONSTRAINT kge_user_fk       FOREIGN KEY (user_id) REFERENCES users (id),
    KEY khatmil_progress_events_assign_idx (assignment_id, recorded_at)
) ENGINE=InnoDB;

-- Satu khataman = 30 juz campaign tuntas & terverifikasi (siklus campaign)
CREATE TABLE khatmil_completions (
    id           BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    campaign_id  BIGINT NOT NULL,
    cycle        SMALLINT NOT NULL,
    completed_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT kcm_cycle_chk CHECK (cycle >= 1),
    CONSTRAINT kcm_campaign_fk FOREIGN KEY (campaign_id) REFERENCES khatmil_campaigns (id) ON DELETE CASCADE,
    UNIQUE KEY kcm_campaign_cycle_uq (campaign_id, cycle)
) ENGINE=InnoDB;
