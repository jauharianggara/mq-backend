# Graph Report - .  (2026-09-16)

## Corpus Check
- 128 files · ~143,018 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 880 nodes · 1288 edges · 75 communities detected
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output
- Edge kinds: contains: 730 · calls: 371 · references: 157 · method: 26 · rationale_for: 4


## Input Scope
- Requested: auto
- Resolved: committed (source: default-auto)
- Included files: 128 · Candidates: 382
- Excluded: 0 untracked · 20996 ignored · 2 sensitive · 0 missing committed
- Recommendation: Use --scope all or graphify.yaml inputs.corpus for a knowledge-base folder.

## Graph Freshness
- Built from Git commit: `5d2621b`
- Compare this hash to `git rev-parse HEAD` before trusting freshness-sensitive graph output.
## God Nodes (most connected - your core abstractions)
1. `ok()` - 37 edges
2. `ok()` - 16 edges
3. `fetch_visit()` - 16 edges
4. `ok()` - 15 edges
5. `visit_out()` - 15 edges
6. `ok()` - 12 edges
7. `ustadz_perm()` - 12 edges
8. `AppState` - 11 edges
9. `ok()` - 10 edges
10. `admin_perm()` - 10 edges

## Surprising Connections (you probably didn't know these)
- None detected - all connections are within the same source files.

## Communities

### Community 0 - "Community 0"
Cohesion: 0.09
Nodes (48): add_blackout(), add_slot(), admin_approve_payout(), admin_force_cancel(), admin_force_complete(), admin_list_payouts(), admin_list_visits(), admin_mark_transferred() (+40 more)

### Community 1 - "Community 1"
Cohesion: 0.09
Nodes (36): admin_force_cancel(), admin_force_complete(), admin_list_visits(), cancel(), complete_visit(), confirm_visit(), contact_open(), create_visit() (+28 more)

### Community 2 - "Community 2"
Cohesion: 0.06
Nodes (32): AdminVisitFilter, BlackoutOut, BlackoutReq, CreateVisitReq, ForceCancelReq, IncomingVisitOut, MessageOut, MyVisitsOut (+24 more)

### Community 3 - "Community 3"
Cohesion: 0.09
Nodes (7): apply_visit_expired(), apply_visit_paid(), issue_invoice_for_visit(), job_confirm_timeout(), job_payment_poll(), mock_url(), refund_visit_to_deposit()

### Community 4 - "Community 4"
Cohesion: 0.14
Nodes (20): assignment_out(), ayat_offset(), campaign_detail(), claim(), create_campaign(), dberr(), ensure_position_in_juz(), job_stale_reminder() (+12 more)

### Community 5 - "Community 5"
Cohesion: 0.20
Nodes (19): access(), answer(), archive(), close(), create(), detail(), fetch_q(), find_by_client_key() (+11 more)

### Community 6 - "Community 6"
Cohesion: 0.17
Nodes (20): activity(), ActivityQ, admin_assign_pembina(), AssignPembinaPath, AssignPembinaReq, campaign_detail(), claim(), create_campaign() (+12 more)

### Community 7 - "Community 7"
Cohesion: 0.22
Nodes (18): answer(), archive(), ArchiveQ, categories(), close(), create(), detail(), idem() (+10 more)

### Community 8 - "Community 8"
Cohesion: 0.20
Nodes (15): change_password(), consume_action_token(), create_action_token(), forgot_password(), get_me(), hash_password(), login(), logout_all() (+7 more)

### Community 9 - "Community 9"
Cohesion: 0.22
Nodes (17): api(), cached(), db_write(), derive_juz_of(), main(), paginate(), parse_range(), 1-141' | '7' | '1,3,5' -> list int (+9 more)

### Community 10 - "Community 10"
Cohesion: 0.20
Nodes (15): add_bookmark(), audio(), AudioQ, AyahQ, ayahs(), delete_bookmark(), get_last_read(), juz_ayahs() (+7 more)

### Community 11 - "Community 11"
Cohesion: 0.21
Nodes (16): accept_adjustment(), AdjustQ, admin_list_balances(), admin_perm(), admin_propose_adjustment(), admin_transactions(), AdminAdjustReq, get_wallet() (+8 more)

### Community 12 - "Community 12"
Cohesion: 0.13
Nodes (11): ActivityEventOut, AssignmentOut, CampaignDetail, CampaignOut, CampaignUpsertReq, ClaimReq, JuzActiveOut, JuzDoneOut (+3 more)

### Community 13 - "Community 13"
Cohesion: 0.30
Nodes (11): admin_create(), admin_delete(), admin_update(), audit(), find_entity(), ok(), pub_announcements_h(), pub_article_h() (+3 more)

### Community 14 - "Community 14"
Cohesion: 0.29
Nodes (12): change_password(), forgot_password(), login(), logout(), logout_all(), me(), ok(), refresh() (+4 more)

### Community 15 - "Community 15"
Cohesion: 0.17
Nodes (1): AppState

### Community 16 - "Community 16"
Cohesion: 0.18
Nodes (10): ChangePasswordReq, ForgotReq, LoginReq, RefreshReq, RegisterReq, RegisterResp, ResetReq, TokenPair (+2 more)

### Community 17 - "Community 17"
Cohesion: 0.22
Nodes (3): CreateInvoice, Invoice, PaymentGateway

### Community 18 - "Community 18"
Cohesion: 0.33
Nodes (10): admin_create(), admin_delete(), admin_update(), get_material(), list_materials(), ListQ, MaterialUpsertReq, ok() (+2 more)

### Community 19 - "Community 19"
Cohesion: 0.27
Nodes (6): detail(), media_presign(), own_audio(), review(), submit(), to_out()

### Community 21 - "Community 21"
Cohesion: 0.33
Nodes (7): audit_list(), dashboard(), ok(), settings_list(), settings_put(), users_list(), users_patch()

### Community 22 - "Community 22"
Cohesion: 0.29
Nodes (5): accept_group(), admin_assign(), notify(), notify_admins(), reject_group()

### Community 23 - "Community 23"
Cohesion: 0.38
Nodes (9): my_progress(), my_submission_detail(), my_submissions(), ok(), page(), PageQ, queue(), review() (+1 more)

### Community 24 - "Community 24"
Cohesion: 0.56
Nodes (9): media, question_assignments, question_categories, question_messages, question_status_history, questions, users, ustadz_profiles (+1 more)

### Community 25 - "Community 25"
Cohesion: 0.49
Nodes (9): user_locations, users, ustadz_visit_services, ustadz_visit_settings, ustadz_visit_status_history, ustadz_visits, visit_messages, visit_reviews (+1 more)

### Community 26 - "Community 26"
Cohesion: 0.44
Nodes (9): media, question_assignments, question_categories, question_messages, question_status_history, questions, users, ustadz_profiles (+1 more)

### Community 27 - "Community 27"
Cohesion: 0.20
Nodes (9): AddBookmarkReq, AudioOut, AyahOut, BookmarkOut, JuzAyahOut, JuzAyahsOut, LastReadOut, PutLastReadReq (+1 more)

### Community 28 - "Community 28"
Cohesion: 0.58
Nodes (8): claim_next(), ensure_cleanup_job(), ensure_khatmil_job(), ensure_visit_jobs(), execute(), finish(), main(), recover_stale()

### Community 29 - "Community 29"
Cohesion: 0.31
Nodes (5): complete_upload(), create_upload(), magic_ok(), max_bytes(), mime_allowed()

### Community 30 - "Community 30"
Cohesion: 0.42
Nodes (8): quran_audio_files, quran_ayahs, quran_juzs, quran_surahs, quran_translations, quran_words, tajwid_ayah_annotations, tajwid_rules

### Community 31 - "Community 31"
Cohesion: 0.42
Nodes (8): quran_audio_files, quran_ayahs, quran_juzs, quran_surahs, quran_translations, quran_words, tajwid_ayah_annotations, tajwid_rules

### Community 32 - "Community 32"
Cohesion: 0.36
Nodes (6): ensure_profile(), get_availability(), get_specializations(), put_availability(), put_specializations(), stats()

### Community 33 - "Community 33"
Cohesion: 0.36
Nodes (5): app_version(), AppVersionQ, ok(), santri_home(), ustadz_home()

### Community 34 - "Community 34"
Cohesion: 0.32
Nodes (3): CurrentUser, OptionalUser, try_current_user()

### Community 35 - "Community 35"
Cohesion: 0.50
Nodes (7): permissions, role_permissions, roles, user_devices, user_roles, user_sessions, users

### Community 36 - "Community 36"
Cohesion: 0.50
Nodes (7): bookmarks, learning_materials, media, quran_ayahs, tajwid_rules, user_reading_progress, users

### Community 37 - "Community 37"
Cohesion: 0.61
Nodes (7): khatmil_campaigns, khatmil_completions, khatmil_juz_assignments, khatmil_participants, khatmil_progress, khatmil_progress_events, users

### Community 38 - "Community 38"
Cohesion: 0.43
Nodes (7): announcements, article_categories, articles, banners, faqs, media, users

### Community 39 - "Community 39"
Cohesion: 0.46
Nodes (7): payout_requests, users, ustadz_availability_slots, ustadz_blackout_dates, ustadz_visits, wallet_transactions, wallets

### Community 40 - "Community 40"
Cohesion: 0.50
Nodes (7): permissions, role_permissions, roles, user_devices, user_roles, user_sessions, users

### Community 41 - "Community 41"
Cohesion: 0.50
Nodes (7): bookmarks, learning_materials, media, quran_ayahs, tajwid_rules, user_reading_progress, users

### Community 42 - "Community 42"
Cohesion: 0.61
Nodes (7): khatmil_campaigns, khatmil_completions, khatmil_juz_assignments, khatmil_participants, khatmil_progress, khatmil_progress_events, users

### Community 43 - "Community 43"
Cohesion: 0.43
Nodes (7): announcements, article_categories, articles, banners, faqs, media, users

### Community 44 - "Community 44"
Cohesion: 0.25
Nodes (7): CategoryOut, CreateQuestionReq, MessageOut, QuestionOut, QuestionThread, RejectReq, SendMessageReq

### Community 45 - "Community 45"
Cohesion: 0.46
Nodes (7): delete_device(), delete_me(), list_devices(), ok(), patch_me(), progress_summary(), register_device()

### Community 48 - "Community 48"
Cohesion: 0.29
Nodes (6): ProgressRow, ReviewOut, ReviewReq, SubmissionDetail, SubmissionOut, SubmitReq

### Community 49 - "Community 49"
Cohesion: 0.67
Nodes (6): media, memorization_progress, memorization_reviews, memorization_submissions, quran_surahs, users

### Community 50 - "Community 50"
Cohesion: 0.67
Nodes (6): media, memorization_progress, memorization_reviews, memorization_submissions, quran_surahs, users

### Community 51 - "Community 51"
Cohesion: 0.38
Nodes (5): CursorPage, CursorPage<T>, pagination_boundary_persis_limit(), pagination_habis(), pagination_memotong_dan_cursor()

### Community 52 - "Community 52"
Cohesion: 0.29
Nodes (1): Storage

### Community 53 - "Community 53"
Cohesion: 0.52
Nodes (6): get_availability(), get_specializations(), ok(), put_availability(), put_specializations(), stats()

### Community 54 - "Community 54"
Cohesion: 0.47
Nodes (2): AppError, error_shape_konsisten()

### Community 55 - "Community 55"
Cohesion: 0.53
Nodes (5): activity_events, audit_logs, scheduled_jobs, settings, users

### Community 56 - "Community 56"
Cohesion: 0.53
Nodes (5): activity_events, audit_logs, scheduled_jobs, settings, users

### Community 57 - "Community 57"
Cohesion: 0.33
Nodes (1): Q

### Community 58 - "Community 58"
Cohesion: 0.33
Nodes (5): DeviceOut, DeviceReq, LastRead, PatchMeReq, ProgressSummary

### Community 59 - "Community 59"
Cohesion: 0.33
Nodes (5): AvailabilityOut, PutAvailabilityReq, PutSpecializationsReq, SpecializationOut, UstadzStats

### Community 60 - "Community 60"
Cohesion: 0.53
Nodes (4): balance(), credit(), debit(), ensure()

### Community 62 - "Community 62"
Cohesion: 0.70
Nodes (4): complete_upload(), create_upload(), get_media(), ok()

### Community 63 - "Community 63"
Cohesion: 0.80
Nodes (4): media, user_profiles, users, ustadz_profiles

### Community 64 - "Community 64"
Cohesion: 0.80
Nodes (4): khatmil_campaigns, khatmil_groups, khatmil_juz_assignments, users

### Community 65 - "Community 65"
Cohesion: 0.80
Nodes (4): media, user_profiles, users, ustadz_profiles

### Community 66 - "Community 66"
Cohesion: 0.40
Nodes (1): Meta

### Community 69 - "Community 69"
Cohesion: 0.83
Nodes (3): main(), one(), q()

### Community 70 - "Community 70"
Cohesion: 0.50
Nodes (2): AppConfig, S3Config

### Community 71 - "Community 71"
Cohesion: 0.50
Nodes (3): CreateUploadReq, MediaOut, UploadOut

### Community 72 - "Community 72"
Cohesion: 0.67
Nodes (3): notification_templates, user_notifications, users

### Community 73 - "Community 73"
Cohesion: 0.67
Nodes (3): notification_templates, user_notifications, users

### Community 75 - "Community 75"
Cohesion: 0.67
Nodes (1): Person

### Community 76 - "Community 76"
Cohesion: 0.67
Nodes (2): MaterialOut, PutProgressReq

### Community 77 - "Community 77"
Cohesion: 1.00
Nodes (2): media, users

### Community 78 - "Community 78"
Cohesion: 1.00
Nodes (2): admin_wallet_adjustments, users

### Community 79 - "Community 79"
Cohesion: 1.00
Nodes (2): media, users

### Community 80 - "Community 80"
Cohesion: 1.00
Nodes (2): db(), main()

### Community 91 - "Community 91"
Cohesion: 1.00
Nodes (1): payments

## Knowledge Gaps
- **134 isolated node(s):** `faqs`, `notification_templates`, `scheduled_jobs`, `faqs`, `notification_templates` (+129 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **Thin community `Community 15`** (1 nodes): `AppState`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 52`** (1 nodes): `Storage`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 54`** (2 nodes): `AppError`, `error_shape_konsisten()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 57`** (1 nodes): `Q`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 66`** (1 nodes): `Meta`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 70`** (2 nodes): `AppConfig`, `S3Config`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 75`** (1 nodes): `Person`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 76`** (2 nodes): `MaterialOut`, `PutProgressReq`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 77`** (2 nodes): `media`, `users`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 78`** (2 nodes): `admin_wallet_adjustments`, `users`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 79`** (2 nodes): `media`, `users`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 80`** (2 nodes): `db()`, `main()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 91`** (1 nodes): `payments`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **What connects `faqs`, `notification_templates`, `scheduled_jobs` to the rest of the system?**
  _134 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Community 0` be split into smaller, more focused modules?**
  _Cohesion score 0.0935374149659864 - nodes in this community are weakly interconnected._
- **Should `Community 1` be split into smaller, more focused modules?**
  _Cohesion score 0.0935374149659864 - nodes in this community are weakly interconnected._
- **Should `Community 2` be split into smaller, more focused modules?**
  _Cohesion score 0.0625 - nodes in this community are weakly interconnected._
- **Should `Community 3` be split into smaller, more focused modules?**
  _Cohesion score 0.09230769230769231 - nodes in this community are weakly interconnected._
- **Should `Community 4` be split into smaller, more focused modules?**
  _Cohesion score 0.14333333333333334 - nodes in this community are weakly interconnected._
- **Should `Community 12` be split into smaller, more focused modules?**
  _Cohesion score 0.125 - nodes in this community are weakly interconnected._