# Research record — Noctune 2026-09-07
Scope: maintainer backlog for commit 2abbe55; personal cross-platform terminal music player; no implementation or external writes.
Plan fallback: update_plan unavailable (attempt returned not a function). Discovery complete; targeted follow-up complete; synthesis complete; deliverable verification complete.
Source classes: repository code at pinned commit; official crate/framework/build/product documentation.
All sources accessed 2026-09-07; web publication dates unknown unless title specifies a month.
## Claim-to-source ledger
- Findings 1: Cargo.toml:62 and src/secrets.rs:19,137; Noctune maintainers, pinned 2026-08-31 commit. keyring authors, keyring 3.6.3 documentation, https://docs.rs/keyring/3.6.3/keyring/ (turn8view0); explicit no default features and mock fallback. High static confidence; actual release dependency graph unverified.
- Findings 2,11: src/app/tick.rs:133 and src/app/scan.rs:31; Noctune maintainers, pinned commit. High code confidence, interactive effects inferred. Next action reproduction, not more web searches.
- Finding 3: src/app/playback.rs:231,271; src/audio.rs:588; src/app/mod.rs:833. No pause restoration in asynchronous route; no interaction run.
- Finding 4: src/spotify/api.rs:195; src/spotify/native.rs:25. Spotify, February 2026 changelog https://developer.spotify.com/documentation/web-api/references/changes/february-2026 (turn1view0); migration guide https://developer.spotify.com/documentation/web-api/tutorials/february-2026-migration-guide (turn3view3, turn5view2); quota modes https://developer.spotify.com/documentation/web-api/concepts/quota-modes (turn3view0); July 2026 changelog https://developer.spotify.com/documentation/web-api/references/changes/july-2026 (turn5view0). Disagreement reconciled: February one-client-ID limit superseded by July 25; Extended Quota excluded from February changes. Authenticated account not tested.
- Finding 5: src/updater.rs:76; release workflow. GitHub Docs, Using artifact attestations, https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations (turn6search0). Digest/authenticity distinction is engineering analysis. No compromise alleged.
- Finding 6: .gitignore and CI/release workflow. Rust Cargo team, Cargo.toml vs Cargo.lock, https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html (turn1view1). Lockfile absent locally; recommendation not bitwise reproducibility guarantee.
- Finding 7: .github/workflows/ci.yml; tests in src/app/playback.rs:1021, src/app/util.rs and src/audio.rs. No quantitative coverage claim.
- Finding 8: src/app/playback.rs:271; src/app/mod.rs:898; src/app/services.rs:1231. Workers not cancelled by dropping receiver; resource impact unmeasured.
- Finding 9: src/ytdlp.rs:236; src/cache.rs:96; src/config.rs:58. Separate metadata pruning vs disk audio; no disk/RAM benchmark.
- Finding 10: src/app/playback.rs:384. Duplicated candidate path through fallback; static counterexample.
- Findings 12,13: src/main.rs:45; src/ytdlp.rs; README.md; src/keybinds.rs:70; src/app/input.rs:829. Product proposals, no user research.
- Finding 14: src/ui and src/app/services.rs. Ratatui maintainers, Testing with insta snapshots, https://ratatui.rs/recipes/testing/snapshots/ (turn1view2); recommendation rather than proved visual defect.
- Finding 15: src/vault.rs:68. JSON decode failure defaulting empty; confirmed statically.
Local canonical URLs: https://github.com/WendellOttoni/Noctune/blob/2abbe55/ plus paths above.
## Discovery and gap matrix
Credentials | manifest + official primary docs | high | release feature resolution missing | inspect actual build graph during implementation.
Playback | parent and independent reviewer reads | high static | no runtime reproduction | fake-engine regression fixtures.
Spotify | code + current first-party migration/change logs | high scoped | account mode unknown | test specified mode with authorized account.
Distribution | code + official attestations/Cargo docs | high | platform installation untested | matrix smoke tests.
Product/UX | implemented commands + README + Ratatui docs | medium recommendation | no users/screens | task-based usability checks later.
## Search log
First wave: official Spotify development-mode changes; Cargo lockfile guidance; Ratatui snapshot testing.
Follow-up: Spotify February migration, May and July changelogs, quota modes; keyring 3.6.3 feature semantics; GitHub build attestations.
Local discovery: structure, actions, manifest, CLI, Spotify, secrets, cache, workers, UI, vault; independent lanes playback reliability and delivery/security.
Critical spot checks: parent re-read queue retain/seek sink flow; parent opened keyring primary docs; Spotify Extended Quota exception and July supersession reconciled.
Stop reason: each included recommendation has primary code evidence or is explicitly a proposal; consequential external contracts supported; next useful evidence requires runtime/account/user trials, not repeated broad searching.
Additional lane findings (not expanded into selected 15): plugin globals/instruction limits and local IPC resource limits. Avoid presenting these as demonstrated remote vulnerabilities.
Artifact: docs/melhorias-noctune-2026-09-07.md. Canonical internal report stored once in report-source.md. Markdown chosen because no artifact-native DOCX/PDF tool exposed; installed python-docx/reportlab/pandoc unavailable. Structural QA passed: 15 numbered findings, 37 hyperlinks, no replacement glyphs, canonical copy identical, repository link paths and line anchors checked. No visual render claim. Only research documents added; source code unchanged.
