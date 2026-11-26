# Workflow

## Core rules

1. **The track's `plan.md` is the source of truth** for what the tasks are and their status. The [project board](https://github.com/users/jordanpg/projects/8) mirrors that status and owns scheduling (priority, size, iteration). If they disagree, update the board to match `plan.md`.
2. **Tests: moderate.** Write tests for engine logic (reader rules, scorer, search, tiling, data format) where they help. They are encouraged, not a gate on starting work. Unit tests can't show how the TTS sounds; experiments on real clips remain the measure of quality.
3. **Measure, don't assume.** A change that affects engine output is checked with `phono eval` (and `phono calibrate` when the reader or scorer changes) before it counts as done.
4. **Never spend credits without asking.** `mise run //tools:synth <exp> --spend` needs the user's explicit go-ahead every time, even mid-track. Dry runs are free and fine.
5. **Small commits.** One logical change per commit, in Conventional Commits format.
6. **Every piece of work has an issue.** Tracks, plan tasks, experiments, bugs and TODOs are GitHub issues on the `phonocompressor` project (see "Issues and the project board").

## Issues and the project board

Tickets are GitHub Issues on `jordanpg/phonocompressor`, tracked on the user project [`phonocompressor` (#8)](https://github.com/users/jordanpg/projects/8), which is linked to the repo. The `gh` CLI needs the `project` scope (`gh auth refresh -h github.com -s project`, run in a normal terminal).

**The repo is public, so issues are public**, even though the board is private. Never put these in an issue, comment or PR:

- secrets (`ELEVENLABS_API_KEY`, fnox config) or account details beyond credit counts;
- the contents of `sample/messages.json` (gitignored reference messages) or audio clips;
- personal details such as accounts or specific channels and people. Naming Twitch chat as the original use case is fine where it gives context.

Issue, comment and PR text follows the same rule as commits: **no AI attribution lines**.

If the board has a draft item (added by hand in the board UI), convert it to an issue before starting work on it.

### What gets an issue

| Work | Issue | Type | Title |
|---|---|---|---|
| Conductor track | Parent issue. If an issue for the work already exists (e.g. #1), the track uses it. | `Feature`, `Bug`, or `Task` (for refactor and chore tracks) | Track title, e.g. `Ship the WASM web tool` |
| Task in a track's `plan.md` | Sub-issue of the track's issue | `Task` | `<phase>.<task> <description>`, e.g. `1.2 Expose compress() via wasm-bindgen` |
| Experiment | Sub-issue if part of a track, else standalone | `Task` | `exp: <what it probes>`, or `expN: …` once it has a number |
| Bug found outside a track | Standalone | `Bug` | What goes wrong |
| TODO left in code | Standalone | `Task` | What is left to do |

### Board fields

| Field | Values | Set |
|---|---|---|
| Status | Backlog, Ready, In progress, In review, Done | The board sets Backlog when an issue is added; then it follows the lifecycle below |
| Priority | P0, P1, P2 | On every issue. Conductor priority: critical → P0, high → P1, medium/low → P2 |
| Size | XS, S, M, L, XL | Relative size, on every issue when it's created |
| Iteration | 2-week iterations, Iteration 1 started 2026-11-24 | On the board when the user schedules work |
| Sub-issues progress | Automatic | Shows a track's completed tasks |

### Status lifecycle

| Conductor state | Board Status | Issue |
|---|---|---|
| Track created (spec and plan written) | Backlog | Parent and task issues open |
| Track chosen to work on next | Ready | — |
| Task marked `[~]` | In progress (task and its track) | — |
| Task implemented and committed, waiting for the user to verify | In review | — |
| User verified, task marked `[x]` | Done | Closed as completed |
| All tasks done, track complete | Done (track) | Parent closed as completed |
| Track archived or deleted, or task dropped | — | Closed as not planned, with a comment saying why |

### Commands

```sh
# Track issue (at /conductor:new-track), unless one already exists
gh issue create --type Feature --project phonocompressor \
  --title "<track title>" --body "<summary of spec.md; paths to spec.md and plan.md>"

# One sub-issue per plan task
gh issue create --type Task --project phonocompressor --parent <track-issue> \
  --title "1.2 Expose compress() via wasm-bindgen" --body "<task and its acceptance criteria>"

# Set a board field (single-select fields take the option name)
gh project item-edit 8 --owner jordanpg \
  --url https://github.com/jordanpg/phonocompressor/issues/<n> --field Status --value "In progress"
gh project item-edit 8 --owner jordanpg --url <issue-url> --field Priority --value P1
gh project item-edit 8 --owner jordanpg --url <issue-url> --field Size --value S

# Close after verification, or drop
gh issue close <n> --comment "Verified: <what was checked>"
gh issue close <n> --reason "not planned" --comment "<why>"

# What's open on the board
gh project item-list 8 --owner jordanpg --query "-status:Done" --limit 200
```

### Where issue numbers are recorded

- **`tracks.md`:** the track's title cell links its issue: `[Ship the WASM web tool](https://github.com/jordanpg/phonocompressor/issues/1)`. Committed Markdown files don't auto-link `#1`, so use the full URL.
- **`plan.md`:** each task line ends with its issue: `- [ ] Task 1.2: Expose compress() via wasm-bindgen ([#15](https://github.com/jordanpg/phonocompressor/issues/15))`.
- **`metadata.json`:** add `"issue": 1` and `"task_issues": {"1.1": 14, "1.2": 15}`.

## Task lifecycle

1. **Select.** Take the next `[ ]` task in `plan.md`; check its dependencies are done.
2. **Mark in progress.** Change `[ ]` to `[~]`. Set the task's issue (and the track's, on its first task) to **In progress**.
3. **Test first, where it fits.** For engine logic, write a failing test that pins the behaviour (e.g. a reader rule, a tiling case, a score ordering). Skip it for docs, config, experiment definitions and exploratory probes.
4. **Implement.** Make the test pass with the simplest change that does.
5. **Refactor.** Apply the style guides; keep `phono-core` I/O-free.
6. **Verify.** Run the quality gates below. For engine changes, compare `phono eval` before and after, and note the numbers.
7. **Record deviations.** If the work differs from the spec, say why in the track's `spec.md` or `plan.md`. If it changes a design decision or produces experiment results, update `docs/PLAN.md`. New follow-up work becomes a new issue, not a note.
8. **Commit the code** with `Refs #<task-issue>` (see below).
9. **Stop for verification.** Set the task's issue to **In review** and pause after **every task**. Wait for the user to check it before starting the next.
10. **Mark done.** Once the user has verified it: change `[~]` to `[x]` in `plan.md`, commit that separately (`chore(conductor): mark task X.Y complete (<track-id>)` with `Refs #<task-issue>`), and close the task's issue with a comment saying what was verified. If the user finds problems, the task goes back to In progress.

## Commits

[Conventional Commits](https://www.conventionalcommits.org/): `type(scope): summary`, imperative mood, lowercase summary, no trailing period, at most about 72 characters.

**No AI attribution lines.** No `Co-Authored-By`, "Generated with", or similar trailers in commit messages or PR bodies.

| Type | Use for |
|---|---|
| `feat` | New behaviour: a reader rule, a CLI command, a profile, a web feature |
| `fix` | A bug fix |
| `perf` | Faster or smaller, same output |
| `refactor` | Restructuring, same output |
| `test` | Tests only |
| `exp` | Experiment definitions, results and their write-ups in PLAN.md |
| `docs` | README, PLAN.md, conductor docs |
| `build` | Cargo, uv, mise, data-build changes |
| `ci` | CI configuration |
| `style` | Formatting only |
| `chore` | Everything else, including plan status updates |

Scopes: `core`, `reader`, `score`, `search`, `g2p`, `model`, `builder`, `translit`, `cli`, `harness`, `wasm`, `web`, `data`, `plan`, `conductor`.

Examples:

```
feat(reader): read trailing capital as a separate letter
exp: exp10 probe of deunicode/unidecode kana and Han differences
fix(translit): restore Hepburn shi/chi/tsu overrides
build(mise): add fmt and lint tasks
```

Within a Conductor track, end the summary with the track id: `feat(wasm): expose compress() binding (wasm-bundle)`.

Reference the issue in a footer: `Refs #15`. Don't use closing keywords (`Closes`, `Fixes`) in commit messages: a task's issue is closed only after the user verifies it, and a closing keyword fires as soon as the commit reaches `main`. If you open a PR, put `Closes #<n>` in the PR body for each issue it finishes. The board's "Pull request merged" workflow then moves them to Done.

```
feat(wasm): expose compress() binding (wasm-bundle)

Refs #15
```

A breaking change to the data file format or the public engine API gets a `!` (e.g. `feat(data)!: …`) and a `BREAKING CHANGE:` footer.

## Code review

Optional; self-review is fine. Before committing, read your own diff with the style guides in mind. Use `/code-review` on larger engine or data-format changes when it would help.

## Quality gates

| Gate | Requirement | Command |
|---|---|---|
| Format | No diffs | `mise run fmt:check` |
| Lint | Clippy with `-D warnings`; ruff clean | `mise run lint` |
| Rust tests | All passing | `cargo test --workspace` |
| Python tests | All passing, if `tools/` changed | `mise run //tools:test` |
| Data build | Still builds, if the builder or data format changed | `mise run data` |
| Engine output | `phono eval` checked before vs. after; regressions explained | `mise run phono eval` |
| Prediction | `phono calibrate` checked, if the reader or scorer changed | `mise run phono calibrate` |
| wasm32 | `phono-core` and `phono-wasm` still build for wasm | `mise run //crates/wasm:check` |
| WASM parity | The binding still gives the CLI's guesses, if the core, `compress --json` or `crates/wasm` changed | `mise run //crates/wasm:parity` |
| Licensing | No GPL code or data in anything shipped. Crates in the `.wasm` only under a license accepted in `crates/wasm/about.toml` | `mise run //crates/wasm:notices` for the `.wasm`'s crates; a manual check for other deps and data |

The wasm32 gate needs the `wasm32-unknown-unknown` target, which `mise install` adds (`targets` on `rust` in the root `mise.toml`). The wasm-bindgen CLI is pinned in `crates/wasm/mise.toml`; the first `//crates/wasm:*` task installs it, or `mise install --monorepo` does.

`phono eval` needs `sample/messages.json`, and `calibrate` needs exported clips; both are gitignored and local. `//crates/wasm:parity` needs `phono.bin` (`mise run data`). If any is missing, say so rather than skipping silently.

In a fresh git worktree, run `git submodule update --init` before the gates: the core's tests read Phoible's feature table from `phoible/`.

To fix formatting: `mise run fmt`.

## Development commands

Run these from the repo root. Workspace tasks (`data`, `phono`, `fmt`, `fmt:check`, `lint`) are in the root `mise.toml`; the harness and the binding have their own, in `tools/` and `crates/wasm/`, and their tasks are named by path. `mise tasks --all` lists every task. Inside a project directory, `mise run :<task>` runs that project's task, and root tasks need `//:` (e.g. `mise run //:phono`).

```sh
# Setup (clone with --recurse-submodules first)
mise install --monorepo                      # every project's tools, not just the root's
mise run data                                # build data/generated/phono.bin (~15 s)

# Use the engine
mise run phono compress 'bad dream'          # -n top, -m codepoint budget
mise run phono score '҈㌀s' 'hundred thousand apartments'
mise run phono read 'ꀠddꏒꀊm'

# Check
mise run fmt:check && mise run lint && cargo test --workspace
mise run phono eval
mise run //crates/wasm:check && mise run //crates/wasm:parity

# Browser binding
mise run //crates/wasm:build                 # target/wasm-pkg (.wasm, JS glue, .d.ts, notices)
mise run //crates/wasm:serve                 # test page at http://127.0.0.1:8000/ (/?bench times it)

# Harness (free)
mise run //tools:fetch-library               # community library to sample/library.json (URL from fnox)
mise run //tools:credits
mise run //tools:synth <exp>                 # dry run: estimated cost
mise run //tools:transcribe <exp>
mise run //tools:report <exp>

# Harness (spends credits: ask first)
mise run //tools:synth <exp> --spend
```

## Experiments

An experiment is its own task (or track), with an `expN: …` issue. It is done when:

1. `tools/experiments/expN.toml` exists with a `max_credits` cap.
2. The dry run's estimate has been shown to the user and they approved spending.
3. Clips are synthesized, transcribed and reported (`tools/results/expN.md`).
4. PLAN.md has a dated write-up: setup, credits spent, results table, and what it decided.
5. The issue is closed with the headline numbers and a pointer to the PLAN.md section. Keep the audio and sample messages out of the issue.

## Workflow diagram

```
select task → mark [~], issue In progress → test (where it fits) → implement → refactor
    → quality gates → commit code (Refs #n) → issue In review → STOP: user verifies
    → mark [x] + commit plan → close issue (Done)
```
