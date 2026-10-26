# MQ — Permission Matrix (v1 seed; Task 2.2 menyempurnakan)

Format: `module.code` → role. Middleware WAJIB `require_permission("...")` — DILARANG `if role ==`.
Sumber kebenaran implementasi: `src/bin/seed.rs` (idempotent).

| Permission | SUPER_ADMIN | ADMIN | MODERATOR | USTADZ | SANTRI | Endpoint inti |
|---|:-:|:-:|:-:|:-:|:-:|---|
| quran.read | ✓ | ✓ | ✓ | ✓ | ✓ | `GET /quran/*` |
| media.upload | ✓ | ✓ | | ✓ | ✓ | `POST /media/uploads*` |
| notification.read.self | ✓ | ✓ | ✓ | ✓ | ✓ | `GET /me/notifications*` |
| account.delete | ✓ | ✓ | ✓ | ✓ | ✓ | `DELETE /me` |
| khatmil.join | | | | | ✓ | join/claim/progress |
| khatmil.read | ✓ | ✓ | | ✓ | ✓ | `GET /khatmil/campaigns*` |
| ustadz.profile.self | | | | ✓ | | `GET/PUT /me/ustadz/*` |
| users.read | ✓ | ✓ | ✓ | | | admin users list |
| users.manage | ✓ | ✓ | | | | admin CRUD user/status |
| notifications.send | ✓ | ✓ | | | | `POST/GET /admin/notifications*` kirim notifikasi |
| khatmil.manage | ✓ | ✓ | | | | CRUD campaign |
| dashboard.view | ✓ | ✓ | | | | admin dashboard |
| settings.manage | ✓ | ✓ | | | | settings key-value |
| audit.read | ✓ | ✓ | | | | audit log query |
| roles.manage | ✓ | | | | | roles & permission matrix |

Catatan:
- ACL media per-konteks (Bagian III keputusan #7) berlaku DI ATAS permission: pemilik/admin saja bisa presign media tertentu — validasi per-relasi di service.
- `MODERATOR` tanpa `users.manage` (read-only identitas utk kebutuhan moderasi).
- Permission role SANTRI vs USTADZ terpisah — role USTADZ tidak otomatis dapat permission santri (kecuali yang eksplisit ✓ di atas).
