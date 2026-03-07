# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Fitness Journal Coach — an automated AI fitness coaching ecosystem written in Rust (backend) and Next.js (dashboard). It fetches health/activity data from Garmin Connect, generates personalized weekly workout plans via Google Gemini AI, uploads them to the Garmin calendar, and communicates with the user through a Signal Messenger bot. A Next.js PWA dashboard provides rich visualization including muscle heatmaps, recovery charts, strength progression tracking, and AI coaching chat.

## Build & Development Commands

### Rust Backend (root directory)
```bash
cargo build                    # Debug build
cargo build --release          # Release build
cargo run -- --api             # Start REST API server (port 3001)
cargo run -- --signal --daemon # Start Signal bot + background daemon
cargo run -- --login           # Interactive Garmin OAuth login
cargo run -- --delete-workouts # Delete all FJ-AI: prefixed workouts from Garmin
cargo run -- --test-upload <file.json>  # Test uploading a workout file
cargo run -- --test-fetch <workout_id>  # Fetch and print a specific workout
cargo run -- --test-fetch-url <url>     # Fetch an arbitrary Garmin URL
cargo run -- --test-refresh    # Test OAuth2 token refresh
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
docker-compose up -d --build   # Build and start all 4 services
```

## Architecture

### Rust Backend (`src/`)
Single binary with multiple runtime modes selected via CLI flags (`clap`):
- `--api` — Axum REST API server for the dashboard
- `--signal` — Signal bot WebSocket listener
- `--daemon` — Background loop (5-min cycle): fetches Garmin data, syncs to SQLite, triggers AI analysis/generation
- `--login` — Interactive Garmin OAuth flow with MFA support
- `--delete-workouts` — Bulk delete AI-managed workouts from Garmin
- `--test-upload`, `--test-fetch`, `--test-fetch-url`, `--test-refresh` — Debug utilities

Key modules:
- **`config.rs`** — `AppConfig` loaded via `figment` (merges `Fitness.toml` → `Fitness.json` → env vars). Supports profiles (`[default]`, `[dry_run]`). Contains all timing config for notifiers, rate limits, and API bind address.
- **`garmin_api.rs`** — Native Rust Garmin Connect API client (OAuth1/OAuth2). Endpoints: activities, exercise sets, training plans, user profile, max metrics, calendar, workouts (CRUD), sleep data, body battery, training readiness, HRV status, RHR trend. Handles automatic OAuth2 token refresh.
- **`garmin_client.rs`** — High-level client wrapping `GarminApi`. Fetches and assembles `GarminResponse` (activities with set details, plans, profile, metrics, scheduled workouts, recovery). Caches responses in SQLite (5-min TTL). Manages AI workout lifecycle: `cleanup_ai_workouts()`, `create_and_schedule_workout()`, `validate_and_fix_strength_workouts()` (checks scheduled workouts match generated specs), `workout_steps_match()`.
- **`garmin_login.rs`** — Garmin SSO login flow: credentials → CSRF ticket → OAuth1 token → OAuth2 exchange. Full MFA support with `login_step_2_mfa()`.
- **`ai_client.rs`** — Gemini API client. Two modes: single-shot `generate_workout()` and multi-turn `chat_with_history()` with system instruction and context injection. Configurable model via `GEMINI_MODEL` env var (default: `gemini-3-flash-preview`). Logs token usage from response metadata. Includes `extract_json_block()` for parsing workout JSON from markdown responses.
- **`coaching.rs`** — `Coach` builds the comprehensive text "brief" (prompt) from Garmin data, profile goals/constraints/equipment, progression history, weekly deltas, adherence tracking, previous plan response (coaching memory), and recent activity analyses. Also contains `generate_smart_plan()` for training plan logic.
- **`bot.rs`** — Signal bot controller:
  - **WebSocket listener** to `signal-cli-rest-api` with note-to-self/syncMessage support and message deduplication (rolling 100-message buffer).
  - **Commands**: `/status` (body battery, sleep, today's plan), `/generate` (trigger full coach pipeline), `/macros <kcal> <protein>` (log nutrition), `/readiness` (AI race readiness assessment).
  - **Free-text conversation**: Gemini-powered chat with persistent history in SQLite. Context-enriched with: body battery, sleep, today's workouts, 7-day activities, 7-day coach feedback, upcoming races/events with countdown, profile goals/constraints/equipment, and top 15 all-time strength PRs. Can auto-schedule workouts from conversational responses.
  - **Scheduled notifiers** (all broadcast to subscribers):
    - Morning Briefing — daily at `morning_message_time`, lists today's workouts
    - Weekly Review — at `weekly_review_day`/`weekly_review_time`, AI-generated volume/recovery analysis
    - Monthly Debrief — at `monthly_review_day`/`monthly_review_time`, month-over-month comparison with peak weights
    - Race Readiness — at `readiness_message_time`, triggers at 14/7/2 days before events, AI assessment with taper advice
    - Strength Validation — at `strength_validation_time`, compares scheduled workouts against `generated_workouts.json` specs and corrects mismatches
  - **`broadcast_message()`** — sends to all `signal_subscribers`
- **`workout_builder.rs`** — Converts AI-generated JSON workout specs into Garmin Connect API payloads. Exercise resolution via fuzzy matching (`strsim::levenshtein`), manual overrides map, and optional exercise DB. Supports strength, cardio, and rest steps with weight/reps/duration/distance.
- **`api.rs`** — Axum REST API with token auth middleware (`x-api-token` header or `Bearer` auth) and per-endpoint rate limiting via `SlidingWindowLimiter`. Atomic file writes for profiles persistence.
- **`db.rs`** — SQLite via `rusqlite` (bundled). Uses `PRAGMA journal_mode = DELETE` and `synchronous = FULL` for Docker compatibility. Tables: `exercise_history`, `ai_chat_log`, `coach_briefs`, `nutrition_log`, `garmin_cache`, `predicted_durations`, `upcoming_analyses`, `activity_analyses`, `recovery_history`. Max 200 chat messages, 64KB per message.
- **`models.rs`** — Shared data types: `GarminResponse`, `GarminActivity` (with `raw_fields` flatten), `ScheduledWorkout` (with `item_type`, `is_race`, `primary_event`), `GarminRecoveryMetrics` (sleep, body battery, training readiness, HRV, RHR trend), `GarminProfile`, `GarminMaxMetrics`, `GarminPlan`, `GarminSetsData`/`GarminSet`/`GarminExercise`, `ExerciseMuscleMap`.
- **`main.rs`** — Entry point with `run_coach_pipeline()` orchestration:
  1. Fetch Garmin data → 2. Save recovery metrics & sync strength sets → 3. Load profile → 4. Auto-analyze recent activities → 5. Fetch coaching memory (previous plan, analyses, weekly deltas) → 6. Build adherence summary → 7. Generate brief → 8. Generate and publish plan (with restart safeguard via `generated_workouts.json`)

### API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/progression` | Exercise progression history with trend points |
| GET | `/api/progression/deltas` | Week-over-week weight/rep comparisons |
| GET | `/api/recovery` | Current recovery metrics (body battery, sleep, HRV, training readiness) |
| GET | `/api/recovery/history` | 30-day recovery history for charts |
| GET | `/api/workouts/today` | Today's completed and planned workouts |
| GET | `/api/workouts/upcoming` | All future scheduled workouts |
| POST | `/api/force-pull` | Clear Garmin cache and force fresh data fetch |
| POST | `/api/generate` | Trigger full AI coach pipeline (rate limited) |
| POST | `/api/predict_duration` | AI-predicted workout duration (cached in DB) |
| POST | `/api/analyze` | AI analysis of a completed activity (cached in DB) |
| POST | `/api/analyze/upcoming` | AI analysis of an upcoming event with full context |
| GET | `/api/chat` | Retrieve coach brief history |
| POST | `/api/chat` | Send message to AI coach (rate limited) |
| GET | `/api/muscle_heatmap` | 14-day muscle group frequency heatmap |
| GET | `/api/profiles` | Read profiles configuration |
| PUT | `/api/profiles` | Update profiles (validated, atomically written) |

### Next.js Dashboard (`dashboard/`)
- **Next.js 16** with App Router, React 19, Tailwind CSS 4, TypeScript
- **`src/app/api/[...path]/route.ts`** — Catch-all API proxy forwarding to Rust backend with allowlisted paths, injecting `FITNESS_API_TOKEN`. Supports GET, POST, PUT.
- **`middleware.ts`** — Basic Auth guard for `/settings` and `/api/profiles` routes. Uses `DASHBOARD_ADMIN_PASSWORD` or `FITNESS_API_TOKEN`/`API_AUTH_TOKEN` as password.
- **Main page components** (`src/app/`):
  - `MuscleMap.tsx` — Body highlighter showing 14-day muscle fatigue via `@mjcdev/react-body-highlighter`
  - `RecoveryHistoryChart.tsx` — Recharts visualization of body battery, sleep score, training readiness, HRV
  - `Chat.tsx` — AI coaching chat interface
  - `GenerateButton.tsx` — Trigger workout generation
  - `AnalyzeButton.tsx` — Analyze a completed activity
  - `AnalyzeUpcomingButton.tsx` — Analyze an upcoming event/race
  - `ForcePullButton.tsx` — Force refresh Garmin data
- **`settings/page.tsx`** — Protected profile configuration UI (goals, equipment, constraints, auto-analyze sports)

### Data Flow
1. Daemon fetches Garmin data (activities, body battery, sleep, HRV, training readiness, RHR trend, scheduled workouts)
2. Syncs strength sets and recovery metrics to SQLite
3. Auto-analyzes recent activities matching `auto_analyze_sports` → broadcasts analysis via Signal
4. `Coach` builds a text brief combining all data + user profile goals + progression + adherence + coaching memory
5. `AiClient` sends brief to Gemini, receives markdown with embedded JSON workout array
6. Workouts are uploaded to Garmin calendar (prefixed `FJ-AI:` for lifecycle management)
7. Signal bot broadcasts summaries to subscribers

### Docker Compose Services
| Service | Container | Purpose |
|---------|-----------|---------|
| `signal-api` | `fitness-coach-signal-api` | `signal-cli-rest-api` in JSON-RPC mode |
| `fitness-coach` | `fitness-coach` | Signal bot + daemon (`--signal --daemon`) |
| `fitness-api` | `fitness-api` | REST API server (`--api`) |
| `fitness-web` | `fitness-web` | Next.js dashboard |

### Configuration
- Primary: `Fitness.toml` with `figment` profile support (`[default]`, `[dry_run]`)
- Fallback: `Fitness.json`, then environment variables
- Docker overrides via `docker-compose.yml` environment section
- User profiles (goals, equipment, constraints, auto_analyze_sports): `profiles.json` (path configurable via `PROFILES_PATH`)
- Signal sensitive vars (`SIGNAL_PHONE_NUMBER`, `SIGNAL_SUBSCRIBERS`) loaded explicitly from env (not merged by figment)

### Key Configuration Fields
| Field | Default | Description |
|-------|---------|-------------|
| `database_url` | `fitness_journal.db` | SQLite database path |
| `signal_api_host` | `fitness-coach-signal-api` | Signal API container hostname |
| `morning_message_time` | `07:00` | Daily workout reminder time |
| `readiness_message_time` | `08:00` | Race readiness check time |
| `weekly_review_day` / `time` | `Sun` / `18:00` | Weekly AI review schedule |
| `monthly_review_day` / `time` | `1` / `18:00` | Monthly AI debrief schedule |
| `strength_validation_time` | `04:00` | Daily strength workout validation |
| `week_start_day` | `Mon` | Week boundary for progression deltas |
| `cors_allowed_origins` | `http://localhost:3000` | Comma-separated CORS origins |
| `api_bind_addr` | `127.0.0.1:3001` | API server bind address |
| `chat_rate_limit_per_minute` | `30` | Max chat API requests per minute |
| `generate_rate_limit_per_hour` | `6` | Max generate API requests per hour |
| `gemini_api_key` | (empty) | Google Gemini API key |
| `fitness_debug_prompt` | `false` | Print full coaching brief to logs |

### Key Conventions
- AI-managed workouts are prefixed with `FJ-AI:` — the system only creates/deletes workouts with this prefix
- Garmin OAuth tokens stored in `secrets/oauth1_token.json` and `secrets/oauth2_token.json`
- SQLite DB uses DELETE journal mode (not WAL) to avoid corruption on Docker bind mounts
- Logging uses `tracing` crate (not `println!`); log level controlled by `RUST_LOG` env var
- Garmin data is cached in SQLite with 5-minute TTL; use `/api/force-pull` or `clear_garmin_cache()` to bypass
- `generated_workouts.json` serves as a restart safeguard — prevents re-generation when container restarts with empty Garmin cache
- AI model configurable via `GEMINI_MODEL` env var (default: `gemini-3-flash-preview`)
- Activity analyses and duration predictions are cached in SQLite to avoid redundant AI calls
- Dashboard proxy allowlists API paths — new endpoints must be added to `ALLOWED_PATHS` in `route.ts`
