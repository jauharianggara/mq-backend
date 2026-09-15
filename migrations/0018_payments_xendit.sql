-- =============================================================
-- MQ Digital Platform — 0018 (MySQL) payments (Xendit)
-- (Bagian V rev 6 — generik subject_type/subject_id, bisa dipakai modul lain)
--  * active_marker GENERATED + UNIQUE(subject,active_marker)
--    => DB-enforced: satu ACTIVE payment attempt per subject
--       (status PENDING/REFUND_REQUESTED/REFUND_PENDING_MANUAL)
-- =============================================================

CREATE TABLE payments (
    id                BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    provider          VARCHAR(20) NOT NULL DEFAULT 'xendit',
    external_id       VARCHAR(100) NOT NULL UNIQUE,  -- visit-{id}-{uuid8}
    xendit_invoice_id VARCHAR(100) NULL UNIQUE,
    amount            BIGINT NOT NULL,
    status            VARCHAR(25) NOT NULL DEFAULT 'PENDING',
    channel           VARCHAR(50) NULL,              -- QRIS/BCA/OVO/... dari webhook
    subject_type      VARCHAR(30) NOT NULL,          -- 'ustadz_visit'
    subject_id        BIGINT NOT NULL,
    paid_at           TIMESTAMP NULL,
    expires_at        TIMESTAMP NULL,
    refund_id         VARCHAR(100) NULL,             -- id refund Xendit (kalau via API)
    refunded_amount   BIGINT NOT NULL DEFAULT 0,
    active_marker     TINYINT GENERATED ALWAYS AS (
                          CASE WHEN status IN ('PENDING','REFUND_REQUESTED','REFUND_PENDING_MANUAL') THEN 1 ELSE NULL END
                      ) STORED,                       -- 1 active attempt per visit (anti invoice ganda)
    raw_callback      JSON NULL,                     -- log webhook terakhir (audit)
    created_at        TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at        TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    CONSTRAINT pay_status_chk CHECK (status IN ('PENDING','PAID','EXPIRED','REFUND_REQUESTED',
        'REFUND_PENDING_MANUAL','REFUNDED','FAILED')),
    CONSTRAINT pay_amount_chk CHECK (amount > 0),
    CONSTRAINT pay_active_uq UNIQUE (subject_type, subject_id, active_marker),  -- << DB-enforced 1 active attempt (UNIQUE otomatis = index)
    KEY pay_subject_idx (subject_type, subject_id),
    KEY pay_status_idx (status),
    KEY pay_pending_idx (status, expires_at)
) ENGINE=InnoDB;
