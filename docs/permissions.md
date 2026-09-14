# MQ — Permission Matrix (v1 seed; Task 2.2 menyempurnakan)

Format: `module.code` → role. Middleware WAJIB `require_permission("...")` — DILARANG `if role ==`.
Sumber kebenaran implementasi: `src/bin/seed.rs` (idempotent).

| Permission | SUPER_ADMIN | ADMIN | MODERATOR | USTADZ | SANTRI | Endpoint inti |
|---|:-:|:-:|:-:|:-:|:-:|---|
| quran.read | ✓ | ✓ | ✓ | ✓ | ✓ | `GET /quran/*` |
| learning.read | ✓ | ✓ | ✓ | ✓ | ✓ | `GET /learning/*` |
| cms.read | ✓ | ✓ | ✓ | ✓ | ✓ | `GET /cms/*` (published) |
| media.upload | ✓ | ✓ | | ✓ | ✓ | `POST /media/uploads*` |
| notification.read.self | ✓ | ✓ | ✓ | ✓ | ✓ | `GET /me/notifications*` |
| account.delete | ✓ | ✓ | ✓ | ✓ | ✓ | `DELETE /me` |
| memorization.submit | | | | | ✓ | `POST /memorization/submissions` |
| khatmil.join | | | | | ✓ | join/claim/progress |
| khatmil.read | ✓ | ✓ | | ✓ | ✓ | `GET /khatmil/campaigns*` |
| question.create | | | | | ✓ | `POST /questions*` |
| ustadz.profile.self | | | | ✓ | | `GET/PUT /me/ustadz/*` |
| memorization.review | | | | ✓ | | `GET /ustadz/memorization/queue`, `POST .../review` |
| question.answer | | | | ✓ | | inbox/answer/publish-request |
| question.moderate | | ✓ | ✓ | | | moderation queue tanya |
| question.publish.moderate | | ✓ | ✓ | | | approve/reject publish |
| users.read | ✓ | ✓ | ✓ | | | admin users list |
| users.manage | ✓ | ✓ | | | | admin CRUD user/status |
| learning.manage | ✓ | ✓ | | | | CRUD learning_materials |
| khatmil.manage | ✓ | ✓ | | | | CRUD campaign |
| cms.manage | ✓ | ✓ | | | | CRUD cms |
| dashboard.view | ✓ | ✓ | | | | admin dashboard |
| settings.manage | ✓ | ✓ | | | | settings key-value |
| audit.read | ✓ | ✓ | | | | audit log query |
| roles.manage | ✓ | | | | | roles & permission matrix |

Catatan:
- ACL media per-konteks (Bagian III keputusan #7) berlaku DI ATAS permission: pemilik/ustadz-berhak/admin saja bisa presign media setoran — validasi per-relasi di service.
- `MODERATOR` tanpa `users.manage` (read-only identitas utk kebutuhan moderasi).
- SANTRI tidak punya `memorization.review` dst — role USTADZ tidak otomatis dapat permission santri (kecuali yang eksplisit ✓ di atas).
