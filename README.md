# Lexica.

> A local-first English dictionary that tries to make itself unnecessary.

[![release](https://img.shields.io/github/v/release/YinLingxiao/Lexica.)](https://github.com/YinLingxiao/Lexica./releases/latest)
[![downloads](https://img.shields.io/github/downloads/YinLingxiao/Lexica./total)](https://github.com/YinLingxiao/Lexica./releases)
![license](https://img.shields.io/github/license/YinLingxiao/Lexica.)
![platform](https://img.shields.io/badge/platform-Windows%20x64-8a5a2b)
![stack](https://img.shields.io/badge/Tauri%202-Rust%20%C2%B7%20SvelteKit%205-2f6f4e)

**Tauri 2 · Rust · SQLite / FTS5 · SvelteKit 5**

Look a word up, read it the way a page is set, then keep the ones worth remembering. The dictionary, notes, bookmarks, and review schedule stay on this machine. Lexica is built for two daily rituals: an instant lookup beside whatever you are reading, and a short study session from a wordbook.

![Lexica home](docs/screenshots/home-light.png)

---

## Download

Windows 10 / 11 (x64). WebView2 is usually already installed.

| File | Size | Notes |
| --- | --- | --- |
| **[Lexica-setup.exe](https://github.com/YinLingxiao/Lexica./releases/latest/download/Lexica-setup.exe)** | ~3.5 MB | **Recommended.** NSIS installer with Start Menu and desktop shortcuts |
| [Lexica.msi](https://github.com/YinLingxiao/Lexica./releases/latest/download/Lexica.msi) | ~5.3 MB | Better for silent or enterprise deploy |

These are permanent `latest` links. A specific version is on [Releases](https://github.com/YinLingxiao/Lexica./releases).

> The installer is currently **unsigned**. Windows SmartScreen may warn about an unknown publisher. Choose **More info → Run anyway**.

---

## What you can do

### Look up a word

The home page is a search field. Type a prefix and the suggestion list opens; arrow keys and Enter pick an entry. Inflected forms fall back to the headword (`running` → `run`). Recent lookups and a few starter words sit under the search box. A word can also be opened directly with `/?word=meticulous`.

The entry opens as a reading page, not a stack of admin cards:

- The headword is shown whole, then splits into syllables (`me·tic·u·lous`). The dots ease open one after another. Turn this off under **Settings → 学习设置 → 自动分音节**; the same switch drives the study card.
- Phonetic transcription and a speaker button. Pronunciation uses the Windows offline English voice (US or UK, with rate 0.8 / 1 / 1.2).
- Frequency ticks, a difficulty bar, and a CEFR tag when the entry has one.
- **Context first** shows one sense and its examples, then **Full entry** opens the rest. **Full entry** in Settings keeps every sense open.
- Each example highlights the headword. Chinese sits under the English line.
- Further down, when the data exists: inflection, mnemonic, root and affix, derivatives, similar words, synonyms, antonyms, and collocations.
- A private note lives beside the entry. Bookmark, remove, copy the entry, or pause review reminders for that word.

In the browser preview those actions are disabled. The desktop app writes them.

### Study a wordbook

**Review** is a study session, not a wall of due cards.

Pick a book in **学习设置**:

| Book | Id |
| --- | --- |
| CET-4, CET-6, 考研, 高考 | `cet4` `cet6` `ky` `gk` |
| TOEFL, IELTS, GRE | `toefl` `ielts` `gre` |
| Oxford 3000 | `oxford` |
| Words you bookmarked | `custom` |
| Words you have looked up | `encountered` |

A group mixes new words and reviews. You set the daily new-word cap (10, 20, 40, 60, or 100), the mix (`mixed`, new first, or review first), and the plan (`standard`, `intensive`, `casual`).

Each card shows:

- A syllable-split headword, phonetic, and speaker.
- Part of speech and CEFR-tagged English senses.
- Eight extension tabs: 例句, 助记, 词根词缀, 派生, 近义词, 反义词, 形近词, 词形变化. Tabs can be turned off or reordered in study settings. 例句 can prefer a standard sentence or an exam sentence, in stored order or at random.
- Chinese can stay visible or wait for a click.
- **Context** mode leads with the example and hides the full definition until you ask for it.

Grade with the brain button or the keyboard:

| Key | Grade | What happens |
| --- | --- | --- |
| Space, Enter, or 1 | 认识 | The word leaves this group |
| 2 | 模糊 | Counted as hazy, then the next word |
| 3 | 忘记 | The word is sent to the back of the group |

The star bookmarks the word. The trash marks it as known and stops scheduling it. A tick bar shows how far through the group you are. **Previous** / **Next** move without grading. **Start practice** opens the in-session cloze after you have browsed the group: first letter, then the English definition, then the Chinese definition. Grading of that cloze stays on this machine even when an example sentence came from AI.

### Words, stats, and appearance

**Words** lists everything you have encountered. Search it, or filter by Saved, Learning, Familiar, Stable, or Paused. Open a row to look the word up again. Pause or resume reminders per word. `/library?filter=bookmarked` deep-links a filter.

**Stats** shows words encountered, how many are familiar or stable, what is due now, a 14-day lookup and review chart, the memory-stage mix, and words that are fading.

**Settings** holds everything that used to live in floating controls:

- **Theme** — Light, Dark, or System. Switching from the moon button in the sidebar plays a circular reveal from the pointer. Reduced-motion skips the animation and just switches.
- **Style theme** — 留白沙龙, 极地群青, or 牛津辞书. These change type, color, and the entry's rules. They are only chosen here.
- **Lookup** — Context first or Full entry.
- **Text size** — Normal or Large. Large bumps reading text.
- **学习设置** — the wordbook, card, plan, voice, example, syllable, and tab options above.
- **Dictionary** — entry count, re-check, or import your own `stardict.db`.
- **DeepSeek example sentences** — optional, off until you turn it on.
- **Shortcuts** — listed in the app: Ctrl K focuses search, arrows and Enter pick a suggestion, U / 2 / 3 step the lookup, Enter submits a practice answer.

The sidebar collapses to icons on a wide window and becomes a top bar plus a bottom tab bar under 760px.

### AI example sentences

Off by default. When a dictionary sense has no example, lookup can ask your endpoint for one short sentence. In review, the cloze prompt can use that sentence. The grade itself is always computed locally.

- Defaults to DeepSeek: `https://api.deepseek.com/v1` and `deepseek-chat`. Any OpenAI-compatible endpoint works.
- The API key is stored in the **Windows Credential Manager**. It is not written to SQLite, localStorage, or logs. The UI only learns whether a key exists.
- Rust checks the response (sense id, a non-empty sentence, the target as a whole word) before the UI sees it.
- Results are cached by word, sense, endpoint, model, and prompt version.
- Timeout is 15 seconds, with no automatic retry. A failure falls back to the local definition or example.

---

## Screenshots

| | |
| --- | --- |
| ![Home](docs/screenshots/home-light.png) | ![Entry](docs/screenshots/word-light.png) |
| Home. Search first, then a few words to start from. | `meticulous`, syllables open, context and the start of the full entry. |
| ![Dark](docs/screenshots/word-dark.png) | ![Review](docs/screenshots/review-fixture.png) |
| The same entry in dark theme at large text size. | Study card for `surgery`: senses, example, and the grade controls. |
| ![Settings](docs/screenshots/ai-settings.png) | ![Narrow](docs/screenshots/settings-mobile.png) |
| Settings: theme, style, dictionary, and optional AI. | The same page at 480px, with the bottom tab bar. |

> These are the **browser read-only preview** (50 seed words, so the dictionary line reads `50 entries · offline`). The desktop app uses the full ECDICT corpus. Layout and interaction match; saving does not. The preview banner at the top is only in the browser.

---

## First run

1. **Dictionary.** Lexica downloads the free ECDICT corpus (~200 MB zip, about 3.4 million entries, MIT) into the app data folder and imports it. Import can take several minutes. You can skip this and import your own `stardict.db` from Settings later.
2. **Voice.** If the speaker button is silent, install an English offline voice in Windows Settings → Time & language → Speech.
3. **Study.** Open Review. The default book is CET-4, 40 new words a day, syllables on. Change any of that under Settings → 学习设置.
4. **AI examples** *(optional).* Enable them in Settings, paste a key, and test the connection.

---

## How it is built

Business rules live in Rust. The frontend is the page, the interaction, and appearance preferences.

```
src-tauri/src/
├── domain/       Pure models (no I/O)
├── dictionary/   FTS5 lookup, suggestions, inflection, roots, ECDICT download
├── memory/       Memory-state projection and the scheduler
├── review/       Study groups, cloze practice, grading
├── library.rs    Bookmarks, notes, the word list, activity
├── ai.rs         Optional example generation
├── database/     Connection, append-only migrations, timestamps
├── commands/     Tauri IPC
├── app.rs        App state and the data directory
└── bin/          Headless import-ecdict tool
```

```
src/
├── routes/       /  ·  /library  ·  /review  ·  /stats  ·  /settings
├── lib/api.ts    IPC wrappers. The browser gets preview stubs.
├── lib/preview.ts    Read-only fixture used by npm run dev and Playwright
├── lib/syllables.ts  Headword splitting, shared by lookup and study
├── lib/tts.ts        System speech
└── lib/components/   WordView, StudyWordCard, SyllableHeadword, settings
```

- `encounters` and completed `reviews` are the **source of truth**. `memory_states` is a **projection** that can be rebuilt.
- Migrations are append-only (`PRAGMA user_version`). The study wordbooks and grades land in `0005_study_system.sql`.
- The database lives in the app data directory. A `db_dir.txt` pointer redirects storage and takes effect after restart.
- Appearance (theme, style edition, text size, reading density, and the whole study preference object) is local to the window and stored in `localStorage` under `lexica.preferences`. It is not part of the dictionary database.

---

## Develop

You need **Node.js**, **Rust (MSVC)**, the **Windows C++ build tools**, and **WebView2**.

```bash
npm install
npm run tauri dev
```

That starts Vite on `http://127.0.0.1:5173` and opens the desktop window. The window is the real app: ECDICT, SQLite, and the credential store.

Frontend only, with the 50-word read-only preview:

```bash
npm run dev
```

---

## Build

### Standalone binary

```bash
npm run build
cd src-tauri
cargo build --release --features custom-protocol --bin lexica
```

Output: `src-tauri/target/release/lexica.exe`.

`custom-protocol` is required for a release build. Without it the binary cannot serve the embedded `build/` assets.

### Windows installers

```bash
npm install
npm run tauri build
```

- `src-tauri/target/release/bundle/msi/Lexica_0.1.0_x64_en-US.msi`
- `src-tauri/target/release/bundle/nsis/Lexica_0.1.0_x64-setup.exe`

Bump `version` in `src-tauri/tauri.conf.json` before shipping.

### Headless ECDICT import

```bash
cargo run --release --bin import-ecdict -- <stardict.db> <lexica.db>
```

This creates and migrates the target database if needed. Running it again is safe.

---

## Publish

> This repo does **not** use GitHub Actions.
>
> The repository name ends with a period (`Lexica.`). Windows runners use `D:\a\<name>\<name>`, and Windows cannot create a directory with a trailing dot, so `actions/checkout` fails before any workflow step runs.

From a machine that already has the toolchain:

```bash
npm run tauri build
node scripts/publish-release.mjs
```

The script reads the version from `tauri.conf.json`, creates or updates the `v<version>` release, and uploads both the versioned artifacts and the stable names (`Lexica-setup.exe`, `Lexica.msi`). `releases/latest/download/...` then points at that build.

Release notes are filled from [`docs/release-notes-template.md`](docs/release-notes-template.md).

```bash
node scripts/publish-release.mjs --skip-build
node scripts/publish-release.mjs --replace
node scripts/publish-release.mjs --tag v0.2.0
```

Auth is `GITHUB_TOKEN`, or the Git credential helper. On a signed-in Windows machine the helper is usually enough.

---

## Test

```bash
npm run check
npm run build
npx playwright install chromium
npm run test:e2e
cd src-tauri
cargo test
cargo clippy --all-targets -- -D warnings
```

- Playwright covers the preview UI and the IPC fixtures: suggestions, bookmarks, study cards, hint order, anti-double-submit, the theme reveal, AI fallback, and the narrow layout. Several of those tests also write `docs/screenshots/`.
- Rust tests cover real SQLite storage and the memory rules.

---

## Privacy

- The dictionary, bookmarks, notes, and review history live in **local** SQLite.
- The network is used for two things only: the **first-run ECDICT download**, and **optional AI examples** after you turn them on.
- With AI on, only the current word and sense are sent, and only to the endpoint you configured. Grading stays local.
- API keys stay in the Windows Credential Manager.

---

## Limits

- Windows x64 only (MSI and NSIS).
- The installer is unsigned, so SmartScreen warns on first launch.
- Pronunciation needs a system English voice pack.
- The first dictionary import needs about 200 MB of download, unless you bring your own `stardict.db`.
- AI examples need your own API key and can cost money on that provider.
- The browser preview cannot save. Use the desktop app for bookmarks, notes, and study progress.
- Syllable splits are a local heuristic plus a small curated list. They are a reading aid, not a phonetic authority.

---

## License

[MIT](LICENSE) © YinLingxiao

Free to use, modify, and distribute, including commercially, with the copyright notice kept.

---

## Credits

Review notes and follow-ups: [PROJECT_REVIEW.md](docs/PROJECT_REVIEW.md).

Dictionary data: [ECDICT](https://github.com/skywind3000/ECDICT) (MIT).
