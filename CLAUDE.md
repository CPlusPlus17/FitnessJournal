# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Fitness Journal Coach — an automated fitness coaching assistant written in Rust. It fetches health/activity data from Garmin Connect, generates weekly workout plans via Google Gemini AI, uploads them to the Garmin calendar, and communicates with the user through a Signal Messenger bot. A Next.js dashboard provides visualization.

## Build & Development Commands

### Rust Backend (root directory)
```bash
cargo build                    # Debug build
cargo build --release          # Release build
cargo run -- --api             # Start REST API server (port 3001)
cargo run -- --signal --daemon # Start Signal bot + background daemon
cargo run -- --login           # Interactive Garmin OAuth login
cargo fmt --all -- --check     # Format check
cargo clippy --all-targets --all-features -- -D warnings  # Lint
cargo test --all-targets       # Run tests
```

### Next.js Dashboard (`dashboard/`)
```bash
cd dashboard
npm install        # Install dependencies
npm run dev        # Dev server (port 3000)
npm run build      # Production build
npm run lint       # ESLint
```

### Full Preflight (both Rust + dashboard)
```bash
./scripts/publish-preflight.sh
```

### Docker
```bash
docker-compose up -d --build   # Build and start all services
```

## Architecture

### Rust Backend (`src/`)
Single binary with multiple runtime modes selected via CLI flags (`clap`):
- `--api` — Axum REST API server for the dashboard
- `--signal` — Signal bot WebSocket listener
- `--daemon` — Background loop (5-min cycle): fetches Garmin data, syncs to SQLite, triggers AI analysis/generation
- `--login` — Interactive Garmin OAuth flow

Key modules:
- **`config.rs`** — `AppConfig` loaded via `figment` (merges `Fitness.toml` → env vars). Supports profiles (`[default]`, `[dry_run]`).
- **`garmin_api.rs`** / **`garmin_client.rs`** — Native Rust Garmin Connect API client with OAuth1/OAuth2 auth, data fetching, workout scheduling. Caches responses in SQLite.
- **`garmin_login.rs`** — Garmin SSO login flow with MFA support, saves tokens to `secrets/`.
- **`ai_client.rs`** — Gemini API client. Sends coaching briefs, extracts JSON workout blocks from AI responses.
- **`coaching.rs`** — Builds the text "brief" (prompt) from Garmin data, profile goals, progression history, and recovery metrics.
- **`bot.rs`** — Signal bot controller (WebSocket to `signal-cli-rest-api`), scheduled notifiers (morning briefing, weekly review, monthly debrief, race readiness).
- **`workout_builder.rs`** — Converts AI-generated JSON workout specs into Garmin Connect API payloads.
- **`api.rs`** — Axum routes (`/api/progression`, `/api/chat`, `/api/generate`, `/api/profiles`, etc.) with token auth middleware and rate limiting.
- **`db.rs`** — SQLite via `rusqlite` (bundled). Stores exercise history, chat logs, coach briefs, recovery metrics, activity analyses, Garmin cache. Uses `PRAGMA journal_mode = DELETE` for Docker compatibility.
- **`models.rs`** — Shared data types for Garmin activities, workouts, plans, recovery metrics.

### Next.js Dashboard (`dashboard/`)
- **Next.js 16** with App Router, React 19, Tailwind CSS 4, TypeScript
- **`src/app/api/[...path]/route.ts`** — Catch-all proxy that forwards requests to the Rust API backend (allowlisted paths only), injecting the `FITNESS_API_TOKEN`.
- **`middleware.ts`** — Basic auth guard for `/settings` and `/api/profiles` routes.
- Main page components: `MuscleMap.tsx` (body highlighter), `RecoveryHistoryChart.tsx` (recharts), `Chat.tsx`, `GenerateButton.tsx`, `AnalyzeButton.tsx`, etc.

### Data Flow
1. Daemon fetches Garmin data (activities, body battery, sleep, HRV, scheduled workouts)
2. Syncs strength sets and recovery metrics to SQLite
3. `Coach` builds a text brief combining all data + user profile goals
4. `AiClient` sends brief to Gemini, receives markdown with embedded JSON workout array
5. Workouts are uploaded to Garmin calendar (prefixed `FJ-AI:` for lifecycle management)
6. Signal bot broadcasts summaries to subscribers

### Configuration
- Primary: `Fitness.toml` with `figment` profile support (`[default]`, `[dry_run]`)
- Fallback: `.env` file (loaded by `dotenvy`)
- Docker overrides via `docker-compose.yml` environment section
- User profiles (goals, equipment, constraints): `profiles.json`

### Key Conventions
- AI-managed workouts are prefixed with `FJ-AI:` — the system only creates/deletes workouts with this prefix
- Garmin OAuth tokens stored in `secrets/oauth1_token.json` and `secrets/oauth2_token.json`
- SQLite DB uses DELETE journal mode (not WAL) to avoid corruption on Docker bind mounts
- Logging uses `tracing` crate (not `println!`); log level controlled by `RUST_LOG` env var
