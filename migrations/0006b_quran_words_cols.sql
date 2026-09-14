-- =============================================================
-- MQ Digital Platform — 0006b (MySQL) quran_words v2 (Task 1.2 v8)
-- Ekspansi kolom WBW: transliteration + text_en tersedia sekarang
-- (sumber quran.com); text_id menyusul via dataset QuranWBW (Phase 2).
-- File 0006 utama sudah memuat definisi final (utk fresh install);
-- file ini ALTER instance existing (dev/test) yang sudah apply 0006.
-- =============================================================

ALTER TABLE quran_words
    ADD COLUMN transliteration VARCHAR(200) NULL AFTER text_uthmani,
    ADD COLUMN text_en TEXT NULL AFTER transliteration;
