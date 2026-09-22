-- 0025: Hapus modul CMS & Materi (learning) — keputusan user 22 Sep 2026.
-- Modul learning (materi pembelajaran) dan CMS (articles/banners/announcements/faqs)
-- dihapus dari platform; mobile tidak pernah memakai endpoint tsb.
-- Idempotent: DROP IF EXISTS + purge permissions by module.

-- Purge RBAC utk modul yang dihapus (seed baru juga tidak lagi menanamnya)
DELETE rp FROM role_permissions rp
JOIN permissions p ON p.id = rp.permission_id
WHERE p.module IN ('learning', 'cms');

DELETE FROM permissions WHERE module IN ('learning', 'cms');

-- Tabel modul learning (0007) — tajwid_ayah_annotations (dormant) di-drop dulu
-- karena mem-FK ke tajwid_rules; quran_words DIPERTAHANKAN (fondasi WBW Phase 2).
DROP TABLE IF EXISTS learning_materials;
DROP TABLE IF EXISTS tajwid_ayah_annotations;
DROP TABLE IF EXISTS tajwid_rules;

-- Tabel modul CMS (0011)
DROP TABLE IF EXISTS articles;
DROP TABLE IF EXISTS banners;
DROP TABLE IF EXISTS announcements;
DROP TABLE IF EXISTS faqs;
