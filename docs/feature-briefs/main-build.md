# Vaexil Mod Manager: Windows builds for Atlas's Reinstall (Feature session brief)

Status: carded 2026-11-18 as Next Up item A in `vmm-project-state.md`; built 2026-11-18 on `feat/windows-builds-for-atlas-s-reinstall` and removed from Next Up there. Fleet: main-build, with Vaexbot C, Studio AE, Pulse B, Keeplight A and Modlist Launcher A.

**Title line:**

```
Vaexil Mod Manager: main-build.yml for Atlas's Reinstall on JASON-PC
```

**Paste as the first message:**

```
Add Vaexil Mod Manager's part of Atlas's "Reinstall on JASON-PC" (Atlas Next Up Z, SSH design phase 6, https://claude.ai/artifact/RZCV7mQTtCHf4nGnwbQt2J; Atlas brief docs/feature-briefs/reinstall-on-jason-pc.md, step 2), and make releases go out only when the owner asks. Read vmm-project-state.md and that brief first. Keeplight A and Modlist Launcher A are the same task in the same Rust, Slint and Inno Setup shape.

Facts, checked 2026-11-18:
- The owner's goal (2026-11-18): cut how often releases go out, while still being able to update every Windows app from main. The owner's decision the same day: once an app has its main build, an official release goes out only when the owner asks for one (replacing the nightly rule of ADR-0026 as updated in agentic-instructions#459), and main builds become the normal way to update JASON-PC.
- release.yml today: on `push: tags: ['v*']`, on windows-latest, dtolnay/rust-toolchain@stable, Inno Setup from choco, `cargo test --workspace --locked`, `./scripts/package.ps1`, then a draft GitHub release with the zip, the setup exe and SHA256SUMS.txt. No schedule. publish-feed.yml runs on `release: published` (the owner publishing the draft), refuses a tag that isn't v<MAJOR.MINOR.PATCH>, and copies `VaexilModManager-<version>-setup.exe` to vaexil-mod-manager-v<version> with vaexil-mod-manager-latest/release.json in jamarq-releases. So the feed already moves only on the owner's action.
- scripts/package.ps1 reads the version from `cargo metadata` (workspace version 0.2.0 on main 77d6126), builds with `cargo build --release --locked -p vaexil-mod-manager -p vmm-cli`, and passes it to ISCC; packaging/installer.iss uses it for AppVersion and `OutputBaseFilename=VaexilModManager-{#AppVersion}-setup`, per user, CloseApplications=no (a running exe is set aside as *.old-<time>, #31).
- Versions: the feed serves 0.2.0 (released 2026-11-14); JASON-PC had 0.2.0 on 2026-11-17 (Atlas's design), installed by the app's own updater.
- The updater installs itself (crates/engine/src/app_update.rs, E8): it reads the feed at most daily or when asked, downloads the installer, checks its SHA-256 and runs it with /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /relaunch=1 after the user confirms. `is_newer()` needs both versions to be exactly three numbers, so "0.2.0+main.abc1234" is not a version to it and nothing is ever newer: a main build would never be offered the next real release. That must be fixed before any main build is installed.
- JAMARQ-Digital-LLC/vaexil-mod-manager is a public repository, so a workflow artifact is downloadable by any signed-in GitHub user, not private as the Atlas design says. The source is public under GPL-3.0, so the exposure is small; say so in the PR. Standard GitHub-hosted runners are free for public repositories.

Build:
1. The updater first: compare only the part before "+" for the current version (the feed's version stays strict MAJOR.MINOR.PATCH in parse()), with tests: an installed X.Y.Z+main.<sha> is not offered X.Y.Z or anything older, is offered X.Y.(Z+1), `known_newer` and `app_update_status` agree, and the downloaded file name stays the release's. The window and `vmm` show the full stamped version.
2. .github/workflows/main-build.yml: workflow_dispatch only, on main. The same build as the release, through the same package.ps1 (no forked copy), with the workspace version stamped `<version>+main.<sha7>` before packaging. `--locked` will refuse a changed workspace version until Cargo.lock matches, so refresh only the workspace entries offline (for example `cargo update --workspace --offline`) or pass the stamp another way; never unlock the dependencies. The committed schema/ and golden files must not change with the stamp. Check what Inno Setup writes for the file version and DisplayVersion with a "+" in AppVersion. Upload the setup exe and SHA256SUMS.txt as an artifact kept 7 days, named so Atlas can find it by commit. No GitHub release and no feed.
3. release.yml: move it from the tag push to workflow_dispatch, which the owner starts; it tags main's head as v<workspace version> and makes the draft as today, refusing a version that already has a tag. publish-feed.yml stays on the owner publishing the draft. Rewrite both header comments to say releases happen only when the owner asks.
4. Read the stamped version back from the built installer and from `vmm --version` (or the command that prints appVersion).

Done when: no schedule or push trigger is left in release.yml, the feed moves only after the owner's dispatch and publish, a dispatch of main-build.yml on your branch produces the artifact with the stamped version read back, the updater tests cover the +main rule, and the PR is open. Never install on a real PC; the first real Reinstall is the owner's, from Atlas, which needs a row in the worker's app table (Atlas Z) and should refuse while a deploy or an SMF update runs, as the app's own updater does (check_can_update_app).

Follow the repo's verify (CONTRIBUTING.md: cargo fmt, clippy with -D warnings, tests, the release build, the schema check and cargo deny, plus actionlint on the workflows), in one PR from your own worktree. Don't install on any machine by hand. Remove item A from `## Next Up` in vmm-project-state.md and update the plan page https://claude.ai/artifact/HBDVnjvCrkyQsY7Bak28Dm before finishing.
```
