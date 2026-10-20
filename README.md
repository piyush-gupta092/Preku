# Preku

A tiny companion that lives in a notch at the top of your screen. Preku keeps your focus timer, tasks, clipboard, system health and gentle wellbeing nudges in one calm place, looked after by an animated mascot called **Pip**.

Everything stays on your computer: no account, no telemetry, no cloud.

## What it does

- **Productivity:** a focus timer, tasks with up to three priorities for today and reminders, clipboard history, music controls, and an optional read-only calendar.
- **System:** CPU, memory, disk, network and battery at a glance, alert rules, and for developers Docker containers, GitHub Actions builds, website checks and `preku watch` for long commands.
- **Wellbeing:** eye, movement and posture breaks at unpredictable moments, water, habits with streaks, screen time and a weekly reflection.
- **Phone:** pair your phone over your own Wi-Fi to share files, text and links both ways.
- **A moment for you:** once a day Pip greets you with a short thought and a word about the weather.
- **Quiet when it matters:** reminders wait during focus sessions, games and presentations.
- **Skills:** add your own thoughts, nudges and sounds, or install ones other people made (see below).

## Install

Download `Preku_<version>_x64-setup.exe` from the releases page and run it. The installer isn't code-signed yet, so Windows SmartScreen asks once: choose **More info → Run anyway**.

---

## Skills

A skill is **one small JSON file** that adds to Preku:

| Adds | Where it shows |
|---|---|
| **Thoughts** | "A moment for you" picks one per day from Preku's own thoughts and those of your skills |
| **Nudges** | Short lines in the notch now and then (stretch your wrists, drink some water…) |
| **Sounds** | Short synthesised chimes for your nudges |

Skills are **data only**. They can't run code, read your data or go online, so a skill from anywhere is safe to install. Preku checks every file before keeping it and tells you exactly what is wrong if it can't install one.

### Use a skill

1. Download a skill file (`.json`), for example from [`skills/community`](skills/community).
2. In Preku open **Settings → Skills → Install a skill…** and pick the file.
3. Switch it on or off any time, or remove it. Installing a newer version of the same skill updates it and keeps your switch.

### Make a skill

Start from an example: [`kind-words.json`](skills/examples/kind-words.json) (thoughts) or [`wrist-and-eye-care.json`](skills/examples/wrist-and-eye-care.json) (nudges and a sound).

```json
{
  "$schema": "https://raw.githubusercontent.com/<owner>/preku/main/skills/schema/skill-1.schema.json",
  "formatVersion": 1,
  "id": "io.github.yourname.stretch-break",
  "name": "Stretch break",
  "version": "1.0.0",
  "author": { "name": "Your Name", "url": "https://github.com/yourname" },
  "description": "A friendly stretch reminder during working hours.",
  "license": "CC0-1.0",
  "contributes": {
    "thoughts": [{ "text": "Your body is part of the work too." }],
    "nudges": [
      {
        "id": "stretch",
        "lines": ["Stand up and reach for the ceiling?", "Roll your shoulders back, twice."],
        "everyMin": 50,
        "random": true,
        "days": [1, 2, 3, 4, 5],
        "between": ["09:00", "18:00"],
        "sound": "chime"
      }
    ],
    "sounds": [
      {
        "id": "chime",
        "name": "Chime",
        "notes": [
          { "freq": 880, "at": 0, "length": 0.5, "gain": 0.12 },
          { "freq": 1318.5, "at": 0.12, "length": 0.6, "gain": 0.08, "wave": "triangle" }
        ]
      }
    ]
  }
}
```

Point `$schema` at [`skills/schema/skill-1.schema.json`](skills/schema/skill-1.schema.json) (a local path works too) and editors such as VS Code autocomplete every field and underline mistakes.

#### The format, version 1

| Field | Rules |
|---|---|
| `formatVersion` | Always `1`. |
| `id` | Reverse-domain, lowercase, 2–6 parts of `a-z 0-9 -` joined by dots, e.g. `io.github.yourname.stretch-break`. **Never change it**: it's how Preku recognises updates. |
| `name` | Up to 40 characters. |
| `version` | Semantic version, e.g. `1.2.0`. Raise it with every published change. |
| `author` | `name` (up to 60 characters) and an optional `https://` `url`. |
| `description` | Up to 200 characters. |
| `license`, `homepage` | Optional. An SPDX license id; an `https://` address. |
| `contributes` | At least one of `thoughts`, `nudges`, `sounds`. |

**Thoughts:** up to 500, each `{ "text": "…", "author": "…" }`. `text` is up to 140 characters, and `author` is optional, for quotes.

**Nudges:** up to 10. Each has these fields:

- `id`: unique within the skill (`a-z 0-9 -`).
- `lines`: 1–20 lines of up to 60 characters, shown in turn.
- `everyMin`: 5–240 minutes of *active* use between two nudges. Time away from the PC doesn't count.
- `random`: defaults to `true`. The nudge lands somewhere between 65% and 125% of `everyMin`, so it never feels mechanical.
- `days`: optional weekdays, `1` = Monday … `7` = Sunday.
- `between`: optional local hours such as `["09:00", "18:00"]`. `["22:00", "06:00"]` spans midnight.
- `sound`: `"default"` (Preku's pop-up sound), `"none"`, or the `id` of one of the skill's sounds.

Nudges wait while the user is away, focusing, presenting or gaming, and leave the notch by themselves after a minute. Two skill nudges are always at least 2 minutes apart, and users can hide them all in Settings.

**Sounds:** up to 5, each `{ "id", "name", "notes" }` with 1–12 notes. A note is `{ "freq": 50–4000 Hz, "at": seconds from the start, "length": 0.01–1.5 s, "gain": 0–0.4, "wave": "sine" | "triangle" | "square" }`. The whole sound ends within 2 seconds. Sounds play only when the user has sounds switched on.

All text is plain: no line breaks, tabs, HTML or Markdown. Unknown fields are refused, so a typo is reported instead of silently ignored. The file can be up to 256 KB.

#### Check your skill

```bash
node skills/tools/validate.mjs my-skill.json
```

This runs the same checks as the app (Node 18 or newer, no install needed). Then install the file in Preku and watch it work. A nudge with `"everyMin": 5, "random": false` is handy while testing.

### Publish a skill

Shared skills live in [`skills/community`](skills/community), one file per skill, named after its id: `io.github.yourname.stretch-break.json`.

1. Fork this repository and add your file to `skills/community/`.
2. Run `node skills/tools/validate.mjs --name-is-id skills/community/<your-file>.json`.
3. Open a pull request. CI validates every shared skill, and a maintainer reviews the words for kindness and the sounds for gentleness.
4. To update your skill, raise `version` and open another pull request. Only the author may update their skill.

Guidelines for shared skills: be kind. No pressure, guilt or shaming. No ads, links that sell or personal data. Keep sounds soft. Only publish thoughts that you wrote or that are free to share (credit quotes with `author`).

### What's next for skills

Format version 1 is deliberately data-only. Planned next steps, each opt-in and announced before it ships:

- an in-app gallery that lists the shared skills;
- new kinds of contributions (for example timer presets or habit packs);
- skills that run code, only inside a sandbox with permissions the user grants one by one.

Every skill that works today will keep working: a new format version will only add, never break.

---

## For developers

```
desktop/    Tauri 2 app for Windows and Linux (Rust in src-tauri/, Svelte 5 + TypeScript in src/)
cli/        `preku` command-line helper (`preku watch <command>`)
android/    Native Android app (Kotlin, Jetpack Compose)
protocol/   Phone ↔ PC message schemas, examples and the codegen script
skills/     Skill format: schema, examples, shared skills and the validator
design/     Source artwork (app icon, Pip)
```

### Prerequisites

- **Node 20+** and **pnpm 9**
- **Rust** (stable) via [rustup](https://rustup.rs). On Windows you also need the *MSVC C++ Build Tools* and WebView2 (part of Windows 11).
- **Linux desktop dependencies:** `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf libdbus-1-dev`
- **Android:** Android Studio (JDK 17, SDK 35)

### Run

```bash
# Desktop
cd desktop
pnpm install
pnpm tauri dev          # opens the app
pnpm lint && pnpm test  # frontend checks
cd src-tauri && cargo test   # backend tests; also regenerates TS types in src/lib/types/generated

# Android: open android/ in Android Studio and run the "playDebug" variant, or:
cd android
gradle wrapper --gradle-version 8.11.1
./gradlew testPlayDebugUnitTest assemblePlayDebug

# preku CLI: get a pop-out when a long command finishes
cd cli
cargo build --release          # binary: cli/target/release/preku(.exe); put it on your PATH
preku watch npm test

# Protocol: after editing protocol/schemas, regenerate the Rust and Kotlin types
node protocol/codegen/generate.mjs
```

> **exFAT drives:** `desktop/.npmrc` makes pnpm copy files instead of symlinking them, because exFAT has no symlinks.

### Build the Windows installer

```
cd desktop
pnpm tauri build --bundles nsis
```

The installer is written to `desktop/src-tauri/target/release/bundle/nsis/Preku_<version>_x64-setup.exe`. The first build downloads the NSIS packaging tool.

## License

The code is MIT licensed. Pip, the name Preku and the sounds are project assets. Skills in `skills/` carry their own license (the official ones are CC0).

Made by Piyush Gupta.
