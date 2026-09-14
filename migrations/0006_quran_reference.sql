-- =============================================================
-- MQ Digital Platform — 0006 (MySQL) Quran Reference (IMPORT-ONLY)
-- Data ini REFERENCE DATA, bukan konten CMS. TIDAK ADA CRUD admin;
-- diisi via scripts/import_quran (Task 1.2):
--   - quran_surahs / quran_ayahs : tanzil.net (teks Uthmani + Imlaei)
--   - quran_translations         : Kemenag (via quran.com API v4 / dataset publik)
--   - quran_audio_files          : metadata audio per-ayat Murattal (everyayah.com) — file di CDN
--   - quran_words                : word-by-word + arti (format QuranWBW) — di-import sejak awal (v8),
--                                  fondasi WBW + tajwid per-token Phase 2; TANPA UI di MVP
-- Upsert berdasarkan natural key (UNIQUE), aman dijalankan ulang.
-- =============================================================

CREATE TABLE quran_surahs (
    id          SMALLINT NOT NULL PRIMARY KEY,       -- natural key 1..114
    name_arabic TEXT NOT NULL,
    name_latin  VARCHAR(100) NOT NULL,
    name_id     VARCHAR(100) NOT NULL,               -- 'Al-Fatihah'
    ayah_count  SMALLINT NOT NULL,
    revelation  VARCHAR(20) NOT NULL,
    CONSTRAINT qs_id_chk CHECK (id BETWEEN 1 AND 114),
    CONSTRAINT qs_revelation_chk CHECK (revelation IN ('MAKKAH','MADINAH'))
) ENGINE=InnoDB;

CREATE TABLE quran_juzs (
    id              SMALLINT NOT NULL PRIMARY KEY,   -- 1..30
    start_surah_id  SMALLINT NULL,
    start_ayah      SMALLINT NULL,
    end_surah_id    SMALLINT NULL,
    end_ayah        SMALLINT NULL,
    CONSTRAINT qj_id_chk CHECK (id BETWEEN 1 AND 30),
    CONSTRAINT qj_start_fk FOREIGN KEY (start_surah_id) REFERENCES quran_surahs (id),
    CONSTRAINT qj_end_fk   FOREIGN KEY (end_surah_id)   REFERENCES quran_surahs (id)
) ENGINE=InnoDB;

CREATE TABLE quran_ayahs (
    id           BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    surah_id     SMALLINT NOT NULL,
    ayah_number  SMALLINT NOT NULL,
    text_uthmani TEXT NOT NULL,                      -- teks resmi mushaf (sumber highlight)
    text_imlaei  TEXT,                               -- ejaan sederhana: sumber search/TTS
    text_latin   TEXT,                               -- transliterasi
    juz          SMALLINT NOT NULL,
    hizb         SMALLINT NULL,
    page         SMALLINT NOT NULL,                  -- halaman mushaf standar (1-604)
    sajda        TINYINT(1) NOT NULL DEFAULT 0,
    CONSTRAINT qa_juz_chk  CHECK (juz BETWEEN 1 AND 30),
    CONSTRAINT qa_sajda_chk CHECK (sajda IN (0,1)),
    CONSTRAINT qa_surah_fk FOREIGN KEY (surah_id) REFERENCES quran_surahs (id),
    UNIQUE KEY qa_surah_ayah_uq (surah_id, ayah_number),
    KEY quran_ayahs_juz_idx (juz),
    KEY quran_ayahs_page_idx (page)
) ENGINE=InnoDB;

CREATE TABLE quran_translations (
    id              BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    ayah_id         BIGINT NOT NULL,
    translator_code VARCHAR(50) NOT NULL,            -- 'KEMENAG', 'QURANCOM_ID'...
    text            TEXT NOT NULL,
    CONSTRAINT qt_ayah_fk FOREIGN KEY (ayah_id) REFERENCES quran_ayahs (id) ON DELETE CASCADE,
    UNIQUE KEY qt_ayah_translator_uq (ayah_id, translator_code),
    KEY quran_translations_code_idx (translator_code)
) ENGINE=InnoDB;

CREATE TABLE quran_audio_files (
    id           BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    ayah_id      BIGINT NOT NULL,
    reciter_code VARCHAR(100) NOT NULL,              -- 'ALAFASY_128KBPS', 'HUSARY_128KBPS'...
    audio_url    VARCHAR(500) NOT NULL,              -- URL CDN / mirror everyayah (bukan media table)
    byte_size    BIGINT NULL,
    duration_ms  INT NULL,
    CONSTRAINT qaf_ayah_fk FOREIGN KEY (ayah_id) REFERENCES quran_ayahs (id) ON DELETE CASCADE,
    UNIQUE KEY qaf_ayah_reciter_uq (ayah_id, reciter_code),
    KEY quran_audio_reciter_idx (reciter_code)
) ENGINE=InnoDB;

CREATE TABLE quran_words (
    id            BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    ayah_id       BIGINT NOT NULL,
    position      SMALLINT NOT NULL,                 -- urutan kata dalam ayat (mulai 1)
    text_uthmani TEXT NOT NULL,
    text_id       TEXT,                              -- arti per kata (id) utk WBW
    CONSTRAINT qw_ayah_fk FOREIGN KEY (ayah_id) REFERENCES quran_ayahs (id) ON DELETE CASCADE,
    UNIQUE KEY qw_ayah_pos_uq (ayah_id, position)
) ENGINE=InnoDB;

-- ----------------------------------------------------------------
-- Tajwid — SKOP MVP: materi per rule + highlight PER-AYAT.
-- tajwid_ayah_annotations DORMANT (kosong, keputusan #3): highlight
-- granular = Phase 2, diposisikan terhadap quran_words (token).
-- ----------------------------------------------------------------

CREATE TABLE tajwid_rules (
    id          SMALLINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    code        VARCHAR(100) NOT NULL UNIQUE,        -- 'IDGHAM_BIGUNNAH', 'IQLAB', ...
    name_id     VARCHAR(200) NOT NULL,
    description TEXT,
    color_hex   CHAR(7),                             -- warna highlight di frontend
    sort_order  SMALLINT NOT NULL DEFAULT 0,
    is_active   TINYINT(1) NOT NULL DEFAULT 1
) ENGINE=InnoDB;

CREATE TABLE tajwid_ayah_annotations (
    id               BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    ayah_id          BIGINT NOT NULL,
    rule_id          SMALLINT NOT NULL,
    occurrence_count SMALLINT NOT NULL DEFAULT 1,
    note             TEXT,
    CONSTRAINT taa_ayah_fk FOREIGN KEY (ayah_id) REFERENCES quran_ayahs (id) ON DELETE CASCADE,
    CONSTRAINT taa_rule_fk FOREIGN KEY (rule_id) REFERENCES tajwid_rules (id),
    CONSTRAINT taa_count_chk CHECK (occurrence_count >= 1),
    UNIQUE KEY taa_ayah_rule_uq (ayah_id, rule_id),
    KEY tajwid_ayah_rule_idx (rule_id)
) ENGINE=InnoDB;
