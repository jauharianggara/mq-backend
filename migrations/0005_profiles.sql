-- =============================================================
-- MQ Digital Platform — 0005 (MySQL) Profil Santri & Ustadz
-- =============================================================

CREATE TABLE user_profiles (
    user_id        BIGINT NOT NULL PRIMARY KEY,
    full_name      VARCHAR(200) NOT NULL,
    gender         VARCHAR(10)  NULL,
    birth_date     DATE NULL,
    address_text   TEXT,
    city           VARCHAR(100),          -- teks sederhana; wilayah leaderboard nyusul Phase 2
    province       VARCHAR(100),
    photo_media_id BIGINT NULL,
    bio            TEXT,
    created_at     TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at     TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    CONSTRAINT up_gender_chk CHECK (gender IN ('MALE','FEMALE') OR gender IS NULL),
    CONSTRAINT up_user_fk FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    CONSTRAINT up_photo_fk FOREIGN KEY (photo_media_id) REFERENCES media (id)
) ENGINE=InnoDB;

CREATE TABLE ustadz_profiles (
    user_id                BIGINT NOT NULL PRIMARY KEY,
    code                   VARCHAR(100) UNIQUE,      -- nomor induk asatidz
    title                  VARCHAR(50),              -- 'Ustadz' / 'Ustadzah'
    bio                    TEXT,
    photo_media_id         BIGINT NULL,
    is_accepting_questions TINYINT(1) NOT NULL DEFAULT 1,
    max_active_questions   SMALLINT  NOT NULL DEFAULT 10,
    verified_at            TIMESTAMP NULL,
    created_at             TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at             TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    CONSTRAINT ux_maxq_chk CHECK (max_active_questions BETWEEN 1 AND 100),
    CONSTRAINT ux_user_fk FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    CONSTRAINT ux_photo_fk FOREIGN KEY (photo_media_id) REFERENCES media (id)
) ENGINE=InnoDB;

-- Spesialisasi ustadz didefinisikan di 0010_questions.sql
-- karena mereferensi question_categories.
