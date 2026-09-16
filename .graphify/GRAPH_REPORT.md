# Graph Report - .  (2026-09-16)

## Corpus Check
- 129 files · ~146,957 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 891 nodes · 1309 edges · 76 communities detected
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output
- Edge kinds: contains: 740 · calls: 382 · references: 157 · method: 26 · rationale_for: 4


## Input Scope
- Requested: auto
- Resolved: committed (source: default-auto)
- Included files: 129 · Candidates: 396
- Excluded: 4 untracked · 20995 ignored · 2 sensitive · 0 missing committed
- Recommendation: Use --scope all or graphify.yaml inputs.corpus for a knowledge-base folder.

## Graph Freshness
- Built from Git commit: `00c44e5`
- Compare this hash to `git rev-parse HEAD` before trusting freshness-sensitive graph output.
## God Nodes (most connected - your core abstractions)
1. `ok()` - 39 edges
2. `ok()` - 16 edges
3. `fetch_visit()` - 16 edges
4. `ok()` - 15 edges
5. `visit_out()` - 15 edges
6. `ustadz_perm()` - 14 edges
7. `ok()` - 12 edges
8. `ok()` - 12 edges
9. `AppState` - 11 edges
10. `ok()` - 10 edges

## Surprising Connections (you probably didn't know these)
- None detected - all connections are within the same source files.

## Communities

### Community 41 - "Community 41"
Cohesion: 0.50
Nodes (7): users, roles, permissions, role_permissions, user_roles, user_devices, user_sessions

### Community 79 - "Community 79"
Cohesion: 1.00
Nodes (2): media, users

### Community 65 - "Community 65"
Cohesion: 0.80
Nodes (4): user_profiles, users, media, ustadz_profiles

### Community 32 - "Community 32"
Cohesion: 0.42
Nodes (8): quran_surahs, quran_juzs, quran_ayahs, quran_translations, quran_audio_files, quran_words, tajwid_rules, tajwid_ayah_annotations

### Community 42 - "Community 42"
Cohesion: 0.50
Nodes (7): user_reading_progress, users, quran_ayahs, bookmarks, learning_materials, tajwid_rules, media

### Community 50 - "Community 50"
Cohesion: 0.67
Nodes (6): memorization_submissions, users, quran_surahs, media, memorization_reviews, memorization_progress

### Community 43 - "Community 43"
Cohesion: 0.61
Nodes (7): khatmil_campaigns, users, khatmil_participants, khatmil_juz_assignments, khatmil_progress, khatmil_progress_events, khatmil_completions

### Community 26 - "Community 26"
Cohesion: 0.44
Nodes (9): question_categories, ustadz_specializations, ustadz_profiles, questions, users, question_messages, media, question_assignments (+1 more)

### Community 44 - "Community 44"
Cohesion: 0.43
Nodes (7): article_categories, articles, media, users, banners, announcements, faqs

### Community 73 - "Community 73"
Cohesion: 0.67
Nodes (3): notification_templates, user_notifications, users

### Community 56 - "Community 56"
Cohesion: 0.53
Nodes (5): audit_logs, users, activity_events, settings, scheduled_jobs

### Community 36 - "Community 36"
Cohesion: 0.50
Nodes (7): users, roles, permissions, role_permissions, user_roles, user_devices, user_sessions

### Community 77 - "Community 77"
Cohesion: 1.00
Nodes (2): media, users

### Community 63 - "Community 63"
Cohesion: 0.80
Nodes (4): user_profiles, users, media, ustadz_profiles

### Community 31 - "Community 31"
Cohesion: 0.42
Nodes (8): quran_surahs, quran_juzs, quran_ayahs, quran_translations, quran_audio_files, quran_words, tajwid_rules, tajwid_ayah_annotations

### Community 37 - "Community 37"
Cohesion: 0.50
Nodes (7): user_reading_progress, users, quran_ayahs, bookmarks, learning_materials, tajwid_rules, media

### Community 49 - "Community 49"
Cohesion: 0.67
Nodes (6): memorization_submissions, users, quran_surahs, media, memorization_reviews, memorization_progress

### Community 38 - "Community 38"
Cohesion: 0.61
Nodes (7): khatmil_campaigns, users, khatmil_participants, khatmil_juz_assignments, khatmil_progress, khatmil_progress_events, khatmil_completions

### Community 24 - "Community 24"
Cohesion: 0.56
Nodes (9): question_categories, ustadz_profiles, users, questions, media, question_messages, ustadz_specializations, question_assignments (+1 more)

### Community 39 - "Community 39"
Cohesion: 0.43
Nodes (7): article_categories, articles, media, users, banners, announcements, faqs

### Community 72 - "Community 72"
Cohesion: 0.67
Nodes (3): notification_templates, user_notifications, users

### Community 55 - "Community 55"
Cohesion: 0.53
Nodes (5): audit_logs, users, activity_events, settings, scheduled_jobs

### Community 25 - "Community 25"
Cohesion: 0.49
Nodes (9): visit_service_types, users, user_locations, ustadz_visit_settings, ustadz_visit_services, ustadz_visits, ustadz_visit_status_history, visit_messages (+1 more)

### Community 91 - "Community 91"
Cohesion: 1.00
Nodes (1): payments

### Community 64 - "Community 64"
Cohesion: 0.80
Nodes (4): khatmil_groups, khatmil_campaigns, users, khatmil_juz_assignments

### Community 78 - "Community 78"
Cohesion: 1.00
Nodes (2): admin_wallet_adjustments, users

### Community 40 - "Community 40"
Cohesion: 0.46
Nodes (7): wallets, users, ustadz_visits, wallet_transactions, payout_requests, ustadz_availability_slots, ustadz_blackout_dates

### Community 80 - "Community 80"
Cohesion: 1.00
Nodes (2): db(), main()

### Community 10 - "Community 10"
Cohesion: 0.22
Nodes (17): api(), cached(), paginate(), step_chapters(), step_verses(), step_trans(), step_pages(), parse_range() (+9 more)

### Community 69 - "Community 69"
Cohesion: 0.83
Nodes (3): q(), one(), main()

### Community 75 - "Community 75"
Cohesion: 0.67
Nodes (1): Person

### Community 29 - "Community 29"
Cohesion: 0.58
Nodes (8): main(), ensure_cleanup_job(), ensure_visit_jobs(), ensure_khatmil_job(), recover_stale(), claim_next(), execute(), finish()

### Community 70 - "Community 70"
Cohesion: 0.50
Nodes (2): AppConfig, S3Config

### Community 52 - "Community 52"
Cohesion: 0.29
Nodes (1): Storage

### Community 17 - "Community 17"
Cohesion: 0.22
Nodes (3): CreateInvoice, PaymentGateway, Invoice

### Community 35 - "Community 35"
Cohesion: 0.32
Nodes (3): CurrentUser, OptionalUser, try_current_user()

### Community 21 - "Community 21"
Cohesion: 0.33
Nodes (7): ok(), dashboard(), settings_list(), settings_put(), audit_list(), users_list(), users_patch()

### Community 16 - "Community 16"
Cohesion: 0.18
Nodes (10): RegisterReq, LoginReq, RefreshReq, UserPublic, TokenPair, RegisterResp, VerifyEmailReq, ForgotReq (+2 more)

### Community 14 - "Community 14"
Cohesion: 0.29
Nodes (12): ok(), register(), login(), refresh(), logout(), logout_all(), verify_email(), resend_verification() (+4 more)

### Community 9 - "Community 9"
Cohesion: 0.20
Nodes (15): hash_password(), verify_password(), user_public(), register(), create_action_token(), consume_action_token(), verify_email(), login() (+7 more)

### Community 13 - "Community 13"
Cohesion: 0.30
Nodes (11): ok(), pub_banners_h(), pub_articles_h(), pub_article_h(), pub_announcements_h(), pub_faqs_h(), find_entity(), audit() (+3 more)

### Community 34 - "Community 34"
Cohesion: 0.36
Nodes (5): ok(), santri_home(), ustadz_home(), AppVersionQ, app_version()

### Community 12 - "Community 12"
Cohesion: 0.13
Nodes (11): CampaignUpsertReq, CampaignOut, JuzSlot, CampaignDetail, ClaimReq, AssignmentOut, ProgressReq, JuzActiveOut (+3 more)

### Community 6 - "Community 6"
Cohesion: 0.17
Nodes (20): ok(), ListQ, list_campaigns(), create_campaign(), update_campaign(), campaign_detail(), join(), claim() (+12 more)

### Community 22 - "Community 22"
Cohesion: 0.29
Nodes (5): notify(), notify_admins(), accept_group(), reject_group(), admin_assign()

### Community 4 - "Community 4"
Cohesion: 0.14
Nodes (20): dberr(), juz_bounds(), juz_total_ayat(), ayat_offset(), ensure_position_in_juz(), pct(), create_campaign(), transition_ok() (+12 more)

### Community 76 - "Community 76"
Cohesion: 0.67
Nodes (2): MaterialOut, PutProgressReq

### Community 18 - "Community 18"
Cohesion: 0.33
Nodes (10): ok(), ListQ, list_materials(), get_material(), put_progress(), MaterialUpsertReq, admin_create(), admin_update() (+2 more)

### Community 71 - "Community 71"
Cohesion: 0.50
Nodes (3): CreateUploadReq, UploadOut, MediaOut

### Community 62 - "Community 62"
Cohesion: 0.70
Nodes (4): ok(), create_upload(), complete_upload(), get_media()

### Community 30 - "Community 30"
Cohesion: 0.31
Nodes (5): mime_allowed(), max_bytes(), magic_ok(), create_upload(), complete_upload()

### Community 48 - "Community 48"
Cohesion: 0.29
Nodes (6): SubmitReq, SubmissionOut, SubmissionDetail, ReviewOut, ProgressRow, ReviewReq

### Community 23 - "Community 23"
Cohesion: 0.38
Nodes (9): ok(), page(), PageQ, submit(), my_submissions(), my_submission_detail(), my_progress(), queue() (+1 more)

### Community 19 - "Community 19"
Cohesion: 0.27
Nodes (6): to_out(), own_audio(), submit(), detail(), media_presign(), review()

### Community 57 - "Community 57"
Cohesion: 0.33
Nodes (1): Q

### Community 45 - "Community 45"
Cohesion: 0.25
Nodes (7): CategoryOut, CreateQuestionReq, QuestionOut, MessageOut, QuestionThread, SendMessageReq, RejectReq

### Community 8 - "Community 8"
Cohesion: 0.22
Nodes (18): ok(), page(), PageQ, ArchiveQ, idem(), categories(), create(), my_questions() (+10 more)

### Community 5 - "Community 5"
Cohesion: 0.20
Nodes (19): transition(), create(), find_by_client_key(), q_from_thread(), fetch_q(), access(), thread(), presign_media() (+11 more)

### Community 27 - "Community 27"
Cohesion: 0.20
Nodes (9): SurahOut, AyahOut, AudioOut, LastReadOut, PutLastReadReq, AddBookmarkReq, BookmarkOut, JuzAyahOut (+1 more)

### Community 11 - "Community 11"
Cohesion: 0.20
Nodes (15): ok(), page(), AyahQ, AudioQ, PageQ, surahs(), ayahs(), audio() (+7 more)

### Community 58 - "Community 58"
Cohesion: 0.33
Nodes (5): PatchMeReq, DeviceReq, DeviceOut, ProgressSummary, LastRead

### Community 46 - "Community 46"
Cohesion: 0.46
Nodes (7): ok(), patch_me(), delete_me(), progress_summary(), register_device(), list_devices(), delete_device()

### Community 59 - "Community 59"
Cohesion: 0.33
Nodes (5): SpecializationOut, PutSpecializationsReq, AvailabilityOut, PutAvailabilityReq, UstadzStats

### Community 53 - "Community 53"
Cohesion: 0.52
Nodes (6): ok(), get_specializations(), put_specializations(), get_availability(), put_availability(), stats()

### Community 33 - "Community 33"
Cohesion: 0.36
Nodes (6): ensure_profile(), get_specializations(), put_specializations(), get_availability(), put_availability(), stats()

### Community 2 - "Community 2"
Cohesion: 0.06
Nodes (33): ts(), ts_o(), NearbyUstadz, SlotsReq, SlotMax, SlotsOut, CreateVisitReq, PaymentOut (+25 more)

### Community 0 - "Community 0"
Cohesion: 0.09
Nodes (50): ok(), paged(), ListQ, NearbyQ, nearby(), SlotsQ, slots(), create_visit() (+42 more)

### Community 3 - "Community 3"
Cohesion: 0.09
Nodes (7): issue_invoice_for_visit(), mock_url(), apply_visit_paid(), apply_visit_expired(), refund_visit_to_deposit(), job_confirm_timeout(), job_payment_poll()

### Community 1 - "Community 1"
Cohesion: 0.09
Nodes (36): dberr(), setting_str(), setting_i64(), visit_enabled(), haversine_km(), notify(), log_history(), notify_admins() (+28 more)

### Community 28 - "Community 28"
Cohesion: 0.24
Nodes (3): accept_adjustment(), reject_adjustment(), notify_adjustment_result()

### Community 7 - "Community 7"
Cohesion: 0.18
Nodes (20): ok(), get_wallet(), TopupReq, topup(), TxQ, transactions(), AdjustQ, admin_list_balances() (+12 more)

### Community 60 - "Community 60"
Cohesion: 0.53
Nodes (4): ensure(), balance(), credit(), debit()

### Community 54 - "Community 54"
Cohesion: 0.47
Nodes (2): AppError, error_shape_konsisten()

### Community 51 - "Community 51"
Cohesion: 0.38
Nodes (5): CursorPage, CursorPage<T>, pagination_memotong_dan_cursor(), pagination_habis(), pagination_boundary_persis_limit()

### Community 66 - "Community 66"
Cohesion: 0.40
Nodes (1): Meta

### Community 15 - "Community 15"
Cohesion: 0.17
Nodes (1): AppState

## Knowledge Gaps
- **137 isolated node(s):** `faqs`, `notification_templates`, `scheduled_jobs`, `faqs`, `notification_templates` (+132 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **Thin community `Community 79`** (2 nodes): `media`, `users`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 77`** (2 nodes): `media`, `users`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 91`** (1 nodes): `payments`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 78`** (2 nodes): `admin_wallet_adjustments`, `users`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 80`** (2 nodes): `db()`, `main()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 75`** (1 nodes): `Person`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 70`** (2 nodes): `AppConfig`, `S3Config`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 52`** (1 nodes): `Storage`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 76`** (2 nodes): `MaterialOut`, `PutProgressReq`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 57`** (1 nodes): `Q`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 54`** (2 nodes): `AppError`, `error_shape_konsisten()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 66`** (1 nodes): `Meta`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 15`** (1 nodes): `AppState`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **What connects `faqs`, `notification_templates`, `scheduled_jobs` to the rest of the system?**
  _137 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Community 12` be split into smaller, more focused modules?**
  _Cohesion score 0.125 - nodes in this community are weakly interconnected._
- **Should `Community 4` be split into smaller, more focused modules?**
  _Cohesion score 0.14333333333333334 - nodes in this community are weakly interconnected._
- **Should `Community 2` be split into smaller, more focused modules?**
  _Cohesion score 0.06060606060606061 - nodes in this community are weakly interconnected._
- **Should `Community 0` be split into smaller, more focused modules?**
  _Cohesion score 0.09098039215686274 - nodes in this community are weakly interconnected._
- **Should `Community 3` be split into smaller, more focused modules?**
  _Cohesion score 0.08831908831908832 - nodes in this community are weakly interconnected._
- **Should `Community 1` be split into smaller, more focused modules?**
  _Cohesion score 0.0935374149659864 - nodes in this community are weakly interconnected._