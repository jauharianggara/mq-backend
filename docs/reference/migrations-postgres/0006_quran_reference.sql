-- =============================================================
-- MQ Digital Platform — 0006 Quran Reference (IMPORT-ONLY)
-- Data ini adalah REFERENCE DATA, bukan konten CMS.
-- TIDAK ADA CRUD admin untuk tabel di bawah ini; diisi via seed/import:
--   - quran_surahs / quran_ayahs : tanzil.net (teks Uthmani + Imlaei)
--   - quran_translations         : Kemenag (via quran.com API v4 / dataset publik)
--   - quran_audio_files          : metadata audio per-ayat Murattal (everyayah.com) — file di CDN
--   - quran_words                : word-by-word (format QuranWBW), opsional tapi
--                                  direkomendasikan di-import sejak awal karena jadi
--                                  fondasi WBW + tajwid per-token di Phase 2.
-- Upsert berdasarkan natural key (UNIQUE constraint), aman dijalankan ulang.
-- =============================================================

CREATE TABLE quran_surahs (
    id          SMALLINT PRIMARY KEY CHECK (id BETWEEN 1 AND 114),  -- natural key
    name_arabic TEXT NOT NULL,
    name_latin  TEXT NOT NULL,
    name_id     TEXT NOT NULL,                -- 'Al-Fatihah'
    ayah_count  SMALLINT NOT NULL,
    revelation  revelation_type NOT NULL
);

CREATE TABLE quran_juzs (
    id            SMALLINT PRIMARY KEY CHECK (id BETWEEN 1 AND 30),
    start_surah_id SMALLINT REFERENCES quran_surahs (id),
    start_ayah     SMALLINT,
    end_surah_id   SMALLINT REFERENCES quran_surahs (id),
    end_ayah       SMALLINT
);

CREATE TABLE quran_ayahs (
    id           BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    surah_id     SMALLINT NOT NULL REFERENCES quran_surahs (id),
    ayah_number  SMALLINT NOT NULL,
    text_uthmani TEXT NOT NULL,               -- teks resmi mushaf (sumber highlight)
    text_imlaei  TEXT,                        -- ejaan sederhana: sumber search/TTS
    text_latin   TEXT,                        -- transliterasi
    juz          SMALLINT NOT NULL CHECK (juz BETWEEN 1 AND 30),
    hizb         SMALLINT,
    page         SMALLINT NOT NULL,           -- halaman mushaf standar (1-604)
    sajda        BOOLEAN  NOT NULL DEFAULT false,
    UNIQUE (surah_id, ayah_number)
);

CREATE INDEX quran_ayahs_juz_idx  ON quran_ayahs (juz);
CREATE INDEX quran_ayahs_page_idx ON quran_ayahs (page);

CREATE TABLE quran_translations (
    id              BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    ayah_id         BIGINT NOT NULL REFERENCES quran_ayahs (id) ON DELETE CASCADE,
    translator_code TEXT NOT NULL,            -- 'KEMENAG', 'QURANCOM_ID'...
    text            TEXT NOT NULL,
    UNIQUE (ayah_id, translator_code)
);

CREATE INDEX quran_translations_code_idx ON quran_translations (translator_code);

CREATE TABLE quran_audio_files (
    id           BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    ayah_id      BIGINT NOT NULL REFERENCES quran_ayahs (id) ON DELETE CASCADE,
    reciter_code TEXT NOT NULL,               -- 'ALAFASY_128KBPS', 'HUSARY_128KBPS'...
    audio_url    TEXT NOT NULL,               -- URL CDN / mirror everyayah (bukan media table)
    byte_size    BIGINT,
    duration_ms  INTEGER,
    UNIQUE (ayah_id, reciter_code)
);

CREATE INDEX quran_audio_reciter_idx ON quran_audio_files (reciter_code);

CREATE TABLE quran_words (
    id            BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    ayah_id       BIGINT NOT NULL REFERENCES quran_ayahs (id) ON DELETE CASCADE,
    position      SMALLINT NOT NULL,          -- urutan kata dalam ayat (mulai 1)
    text_uthmani  TEXT NOT NULL,
    text_id       TEXT,                       -- arti per kata (id) utk WBW
    UNIQUE (ayah_id, position)
);

-- ----------------------------------------------------------------
-- Tajwid — SKOP MVP: highlight PER-AYAT.
-- Admin read-only terhadap data ini; materi penjelasan ada di
-- learning_materials (0007). Anotasi per-karakter/per-token DITUNDA
-- ke Phase 2 (offset karakter Uthmani rapuh terhadap Unicode/diakritik);
-- kalau nanti dibutuhkan, posisikan terhadap quran_words (token), bukan
-- offset byte/karakter mentah.
-- ----------------------------------------------------------------

CREATE TABLE tajwid_rules (
    id          SMALLINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    code        TEXT NOT NULL UNIQUE,         -- 'IDGHAM_BIGUNNAH', 'IQLAB', ...
    name_id     TEXT NOT NULL,
    description TEXT,
    color_hex   CHAR(7),                      -- warna highlight di frontend
    sort_order  SMALLINT NOT NULL DEFAULT 0,
    is_active   BOOLEAN NOT NULL DEFAULT true
);

CREATE TABLE tajwid_ayah_annotations (
    id               BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    ayah_id          BIGINT NOT NULL REFERENCES quran_ayahs (id) ON DELETE CASCADE,
    rule_id          SMALLINT NOT NULL REFERENCES tajwid_rules (id),
    occurrence_count SMALLINT NOT NULL DEFAULT 1 CHECK (occurrence_count >= 1),
    note             TEXT,
    UNIQUE (ayah_id, rule_id)
);

CREATE INDEX tajwid_ayah_rule_idx ON tajwid_ayah_annotations (rule_id);
