# FastGTrack

**FastGTrack** is an open-source gym tracker built around a simple idea: **training logs should feel instant**.

No server sync. No waiting. No bloated feature soup. No distractions while you're trying to train.

A lot of fitness apps try to be everything at once: coach, social network, content platform, habit app, and subscription funnel. FastGTrack is intentionally trying to be the opposite — a **fast, focused tool for people who already know how they want to train** and just want to log it with as little friction as possible.

## Philosophy

FastGTrack is built for lifters who want speed and clarity.

That means:

- **Local-first by default** — your training data lives on your device.
- **Fast interaction** — logging sets and moving through a workout should feel immediate.
- **Focused scope** — the app is for tracking training, not trying to replace your judgment.
- **No fake complexity** — no feed, no fluff, no constant prompts trying to pull your attention away.
- **Open source and open to feedback** — the project is meant to improve through real use, honest criticism, and community contributions.

## Who it is for

FastGTrack is **not primarily built for complete beginners**.

It does not try to generate your entire fitness identity for you or tell you what your goals should be. Instead, it is aimed at people who already have a rough idea of how they want to train and want software that stays out of the way.

If you already know your split, your exercises, your sets, your reps, and your progression style, FastGTrack should help you get that into a clean workflow quickly.

## Why FastGTrack exists

Most gym apps feel overloaded.

They often push:

- coaching systems you didn't ask for,
- social features you won't use mid-workout,
- subscription upsells,
- too many abstractions,
- and slow workflows that make logging feel heavier than the actual training.

FastGTrack exists because a gym tracker should first succeed at one thing:

> helping you record your training with as little friction as possible.

Everything else is secondary.

## AI / ChatGPT plan import

One feature direction I care about a lot is the ability to **import training plans from ChatGPT directly**.

That already seems more useful to me than most built-in “AI coach” features in existing fitness apps.

The idea is simple:

- you create or refine a training plan however you want,
- you paste or import it into FastGTrack,
- and the app turns it into something practical you can actually run in the gym.

FastGTrack is not trying to pretend it knows better than you. It should be a tool that helps you execute your plan quickly.

## Built with strong direction, not autopilot

This project is written largely with **Codex assistance**, but not without direction.

The product decisions, workflow ideas, UX priorities, scope, and overall philosophy are still shaped intentionally. Codex helps speed up implementation, but it does **not** replace having a clear opinion about what the app should be.

FastGTrack is opinionated on purpose:

- speed matters,
- friction matters,
- focus matters,
- and software should respect the moment you're actually training.

## Open source and not planned for monetization

FastGTrack is intended to stay:

- **open source**,
- **open to ideas**,
- **open to contributions**,
- and useful for people who want a better gym tracker without the usual bloat.

I do **not** currently plan to monetize it.

The goal is to build something genuinely useful first and keep the project accessible to people who care about a cleaner training experience.

## Current status

There is currently an **Android version** of FastGTrack available in this repository for testing and feedback.

If you want to try it, suggest ideas, contribute code, or just give honest criticism, that would be genuinely appreciated.

## Tech stack

FastGTrack is currently built with:

- **Rust** for application logic,
- **Slint** for the UI,
- **SQLite** for local storage.

This project is designed around a local-first architecture rather than a server-backed sync model.

## Development

### Run locally

```bash
cargo run
```

### Build checks

```bash
cargo check
```

### Android build

There are helper scripts in `scripts/` for Android packaging:

```bash
./scripts/build-android-apk.sh
```

> Depending on your environment, Android tooling and PowerShell may need to be installed and configured.

## Contributing

Contributions, ideas, bug reports, and criticism are all welcome.

If you want to help, useful contributions include:

- improving the workout logging flow,
- polishing the UX,
- fixing bugs,
- refining local-first storage and import/export flows,
- improving Android usability,
- and helping shape ChatGPT/import-based workout creation.

If you open an issue or pull request, clarity and honesty are more useful than trying to be polite. If something feels slow, confusing, unnecessary, or badly designed, that kind of feedback is valuable.

## Roadmap direction

The exact roadmap will evolve, but the general direction is:

- make workout tracking faster,
- keep the UI focused,
- improve plan creation/import,
- preserve local-first ownership of data,
- and avoid turning the app into a bloated all-in-one fitness platform.

## Links

- **Repository:** _add your GitHub repository link here_
- **Android build / releases:** _add your GitHub release or APK link here_

---

If FastGTrack sounds useful to you, feel free to test it, contribute, or send blunt feedback.
