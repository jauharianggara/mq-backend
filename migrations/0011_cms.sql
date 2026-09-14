-- =============================================================
-- MQ Digital Platform — 0011 (MySQL) CMS basic (MVP)
-- =============================================================

CREATE TABLE article_categories (
    id   SMALLINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    slug VARCHAR(100) NOT NULL UNIQUE,
    name VARCHAR(150) NOT NULL
) ENGINE=InnoDB;

CREATE TABLE articles (
    id              BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    slug            VARCHAR(200) NOT NULL UNIQUE,
    category_id     SMALLINT NULL,
    title           VARCHAR(255) NOT NULL,
    excerpt         TEXT,
    content_html    MEDIUMTEXT NOT NULL,            -- sanitasi HTML di service
    cover_media_id  BIGINT NULL,
    author_id       BIGINT NULL,
    status          VARCHAR(50) NOT NULL DEFAULT 'DRAFT',
    published_at    TIMESTAMP NULL,
    reading_minutes SMALLINT NULL,
    view_count      INT NOT NULL DEFAULT 0,
    created_at      TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at      TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    CONSTRAINT art_status_chk CHECK (status IN ('DRAFT','PUBLISHED','ARCHIVED')),
    CONSTRAINT art_category_fk FOREIGN KEY (category_id) REFERENCES article_categories (id),
    CONSTRAINT art_cover_fk   FOREIGN KEY (cover_media_id) REFERENCES media (id),
    CONSTRAINT art_author_fk  FOREIGN KEY (author_id) REFERENCES users (id),
    KEY articles_published_idx (status, published_at DESC)
) ENGINE=InnoDB;

CREATE TABLE banners (
    id             BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    title          VARCHAR(255) NOT NULL,
    image_media_id BIGINT NOT NULL,
    position       VARCHAR(30) NOT NULL DEFAULT 'HOME_TOP',
    target_type    VARCHAR(20),                     -- 'URL' | 'DEEPLINK'
    target_value   VARCHAR(500),
    sort_order     SMALLINT NOT NULL DEFAULT 0,
    starts_at      TIMESTAMP NULL,
    ends_at        TIMESTAMP NULL,
    is_active      TINYINT(1) NOT NULL DEFAULT 1,
    created_at     TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT bn_position_chk CHECK (position IN ('HOME_TOP','HOME_MID','KHOTMIL_TOP')),
    CONSTRAINT bn_image_fk FOREIGN KEY (image_media_id) REFERENCES media (id)
) ENGINE=InnoDB;

CREATE TABLE announcements (
    id         BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    title      VARCHAR(255) NOT NULL,
    body       TEXT NOT NULL,
    level      VARCHAR(20) NOT NULL DEFAULT 'INFO',
    starts_at  TIMESTAMP NULL,
    ends_at    TIMESTAMP NULL,
    is_active  TINYINT(1) NOT NULL DEFAULT 1,
    created_by BIGINT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT an_level_chk CHECK (level IN ('INFO','WARNING','CRITICAL')),
    CONSTRAINT an_creator_fk FOREIGN KEY (created_by) REFERENCES users (id)
) ENGINE=InnoDB;

CREATE TABLE faqs (
    id         BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    question   VARCHAR(500) NOT NULL,
    answer     TEXT NOT NULL,
    sort_order SMALLINT NOT NULL DEFAULT 0,
    is_active  TINYINT(1) NOT NULL DEFAULT 1,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
) ENGINE=InnoDB;
