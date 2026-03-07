# Fitness Journal Dashboard

The web dashboard for [Fitness Journal Coach](../README.md) — a Next.js 16 application providing rich visualization of your AI-coached training data.

## Tech Stack

- **Next.js 16** with App Router
- **React 19**
- **Tailwind CSS 4**
- **TypeScript**
- **Recharts** for data visualization
- **@mjcdev/react-body-highlighter** for the interactive muscle heatmap

## Features

| Component | Description |
|-----------|-------------|
| **Muscle Map** | Interactive body highlighter showing 14-day muscle training frequency |
| **Recovery History** | Recharts visualization of body battery, sleep score, training readiness, and HRV |
| **Strength Progression** | Exercise-by-exercise weight/rep tracking with week-over-week deltas |
| **AI Chat** | Conversational coaching interface connected to the Rust backend |
| **Analyze Button** | AI analysis of any completed Garmin activity |
| **Analyze Upcoming** | AI readiness assessment for upcoming races/events |
| **Generate Button** | Trigger AI workout plan generation |
| **Force Pull** | Force refresh of Garmin data cache |
| **Settings** | Profile management (goals, equipment, constraints) — protected by Basic Auth |

## Architecture

The dashboard communicates with the Rust API backend through a **catch-all API proxy** (`src/app/api/[...path]/route.ts`). This proxy:
- Allowlists specific API paths for security
- Injects the `FITNESS_API_TOKEN` for backend authentication
- Supports GET, POST, and PUT methods

A **middleware** (`middleware.ts`) protects `/settings` and `/api/profiles` routes with HTTP Basic Auth.

## Getting Started

### Prerequisites
- The Rust API backend must be running (`cargo run -- --api` or via Docker)

### Development
```bash
npm install   # Install dependencies (first time only)
npm run dev   # Start dev server at http://localhost:3000
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `FITNESS_API_BASE_URL` | `http://fitness-api:3001` | Rust backend URL |
| `FITNESS_API_TOKEN` | — | API auth token (must match backend's `api_auth_token`) |
| `DASHBOARD_ADMIN_USERNAME` | `admin` | Basic Auth username for settings |
| `DASHBOARD_ADMIN_PASSWORD` | — | Basic Auth password (falls back to `FITNESS_API_TOKEN`) |

### Production Build
```bash
npm run build
npm start
```

### Docker
The dashboard is built and served as part of the `fitness-web` service in the root `docker-compose.yml`.
