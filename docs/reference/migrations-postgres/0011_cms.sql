-- =============================================================
-- MQ Digital Platform — 0011 CMS basic (MVP)
-- =============================================================

CREATE TABLE article_categories (
    id   SMALLINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL
);

CREATE TABLE articles (
    id              BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    slug            TEXT NOT NULL UNIQUE,
    category_id     SMALLINT REFERENCES article_categories (id),
    title           TEXT NOT NULL,
    excerpt         TEXT,
    content_html    TEXT NOT NULL,          -- sanitasi HTML di service
    cover_media_id  BIGINT REFERENCES media (id),
    author_id       BIGINT REFERENCES users (id),
    status          article_status NOT NULL DEFAULT 'DRAFT',
    published_at    TIMESTAMPTZ,
    reading_minutes SMALLINT,
    view_count      INTEGER NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX articles_published_idx ON articles (published_at DESC) WHERE status = 'PUBLISHED';

CREATE TABLE banners (
    id           BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    title        TEXT NOT NULL,
    image_media_id BIGINT NOT NULL REFERENCES media (id),
    position     banner_position NOT NULL DEFAULT 'HOME_TOP',
    target_type  TEXT,                       -- 'URL' | 'DEEPLINK'
    target_value TEXT,
    sort_order   SMALLINT NOT NULL DEFAULT 0,
    starts_at    TIMESTAMPTZ,
    ends_at      TIMESTAMPTZ,
    is_active    BOOLEAN     NOT NULL DEFAULT true,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE announcements (
    id         BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    title      TEXT NOT NULL,
    body       TEXT NOT NULL,
    level      announcement_level NOT NULL DEFAULT 'INFO',
    starts_at  TIMESTAMPTZ,
    ends_at    TIMESTAMPTZ,
    is_active  BOOLEAN     NOT NULL DEFAULT true,
    created_by BIGINT REFERENCES users (id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE faqs (
    id         BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    question   TEXT NOT NULL,
    answer     TEXT NOT NULL,
    sort_order SMALLINT NOT NULL DEFAULT 0,
    is_active  BOOLEAN     NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
