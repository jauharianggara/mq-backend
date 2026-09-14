-- =============================================================
-- MQ Digital Platform — 0007 (MySQL) Learning (reading progress & materi)
-- Progress MEMBACA dipisah tegas dari progress HAFALAN (0008).
-- =============================================================

-- State "terakhir dibaca" — 1 baris per user, hot path di Home screen
CREATE TABLE user_reading_progress (
    user_id      BIGINT NOT NULL PRIMARY KEY,
    last_ayah_id BIGINT NULL,
    last_page    SMALLINT NULL,
    updated_at   TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    CONSTRAINT urp_user_fk FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    CONSTRAINT urp_ayah_fk FOREIGN KEY (last_ayah_id) REFERENCES quran_ayahs (id)
) ENGINE=InnoDB;

CREATE TABLE bookmarks (
    id         BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    user_id    BIGINT NOT NULL,
    ayah_id    BIGINT NOT NULL,
    note       TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT bm_user_fk FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    CONSTRAINT bm_ayah_fk FOREIGN KEY (ayah_id) REFERENCES quran_ayahs (id),
    UNIQUE KEY bm_user_ayah_uq (user_id, ayah_id),
    KEY bookmarks_user_idx (user_id, created_at DESC)
) ENGINE=InnoDB;

-- Materi pembelajaran (termasuk halaman materi tajwid per rule —
-- pengganti MVP untuk tajwid warna per-karakter).
CREATE TABLE learning_materials (
    id             BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    slug           VARCHAR(200) NOT NULL UNIQUE,
    title          VARCHAR(255) NOT NULL,
    tajwid_rule_id SMALLINT NULL,                    -- NULL = materi umum
    content_md     MEDIUMTEXT NOT NULL,              -- markdown
    cover_media_id BIGINT NULL,
    status         VARCHAR(50) NOT NULL DEFAULT 'DRAFT',
    published_at   TIMESTAMP NULL,
    created_by     BIGINT NULL,
    created_at     TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at     TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    CONSTRAINT lm_status_chk CHECK (status IN ('DRAFT','PUBLISHED','ARCHIVED')),
    CONSTRAINT lm_rule_fk FOREIGN KEY (tajwid_rule_id) REFERENCES tajwid_rules (id),
    CONSTRAINT lm_cover_fk FOREIGN KEY (cover_media_id) REFERENCES media (id),
    CONSTRAINT lm_creator_fk FOREIGN KEY (created_by) REFERENCES users (id),
    KEY learning_materials_published_idx (status, published_at DESC)
) ENGINE=InnoDB;
