-- Hapus fitur Setoran Hafalan (memorization) & Tanya Ustadz (questions).
-- question_categories DIPERTAHANKAN: masih dipakai ustadz_specializations
-- (kategori keahlian ustadz untuk profil & pencarian ustadz).
-- Kolom ustadz_profiles.is_accepting_questions / max_active_questions juga dihapus
-- (khusus fitur tanya; seed baru sudah tidak menulisnya).

SET FOREIGN_KEY_CHECKS = 0;

-- memorization
DROP TABLE IF EXISTS memorization_reviews;
DROP TABLE IF EXISTS memorization_submissions;
DROP TABLE IF EXISTS memorization_progress;

-- questions (questions <-> question_messages saling ber-FK: published_message_id)
DROP TABLE IF EXISTS question_status_history;
DROP TABLE IF EXISTS question_assignments;
DROP TABLE IF EXISTS question_messages;
DROP TABLE IF EXISTS questions;

SET FOREIGN_KEY_CHECKS = 1;

-- permissions fitur yang dihapus (role_permissions ikut ON DELETE CASCADE)
DELETE FROM role_permissions
WHERE permission_id IN (SELECT id FROM permissions
                        WHERE code IN ('memorization.submit', 'memorization.review',
                                       'question.create', 'question.answer',
                                       'question.moderate', 'question.publish.moderate'));
DELETE FROM permissions
WHERE code IN ('memorization.submit', 'memorization.review',
               'question.create', 'question.answer',
               'question.moderate', 'question.publish.moderate');

ALTER TABLE ustadz_profiles DROP COLUMN is_accepting_questions;
ALTER TABLE ustadz_profiles DROP COLUMN max_active_questions;
