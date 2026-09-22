-- 0026: Cover image khatmil campaign (user request 22 Sep 2026).
-- Upload via modul media (presign) oleh admin; list/detail men-generate presigned URL.
ALTER TABLE khatmil_campaigns
  ADD COLUMN cover_media_id BIGINT NULL AFTER description,
  ADD CONSTRAINT kc_cover_fk FOREIGN KEY (cover_media_id) REFERENCES media (id);
