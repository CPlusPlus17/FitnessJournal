# Fitness Journal Coach

This project is an automated fitness coaching assistant. It fetches health and activity data from Garmin Connect, sleep and body battery metrics, and syncs past workouts to a local database. Using a large language model (Gemini 3.1 Pro), it generates weekly workout schedules tailored to your goals and automatically uploads them back to your Garmin calendar.

You interact with the coach via a Signal Messenger bot.

## Prerequisites

To run this application, you will need:

- **Docker and Docker Compose** installed on your system.
- **A Signal Phone Number**: You must have a registered phone number to act as the bot. It does not need to be a mobile number; prepaid SIMs or landlines/VoIP numbers work via voice verification.
- **Garmin Connect Account**: Active Garmin credentials to fetch and push data.
- **Google Gemini API Key**: Used to generate the workout plans.

## Setup Instructions

### 1. Environment Configuration

Clone the repository and set up your core configuration files:

1. Create a `.env` file in the root directory:
   ```env
   # Your Gemini Configuration
   GEMINI_API_KEY=your_gemini_api_key

   # The phone number the bot will use, including country code (e.g., +41796000000)
   SIGNAL_PHONE_NUMBER=your_bot_phone_number
   ```

2. Log in to Garmin to generate your tokens:
   ```bash
   cargo run -- --login
   ```
   Follow the interactive prompts to enter your Garmin email, password, and MFA code (if applicable). This will securely authenticate via Garmin SSO and save the resulting OAuth tokens in `secrets/oauth1_token.json` and `secrets/oauth2_token.json`.

### 2. Setting Up the Signal Bot

The Signal API runs in its own container (`signal-cli-rest-api`). You must register your designated bot phone number with the Signal network before starting the main application.

```bash
# Start ONLY the Signal API container
docker-compose up -d signal-api
```

Once it's running, you have two ways to register:

#### Option A: Using the Swagger UI (Recommended)
1. Open your browser and navigate to `http://127.0.0.1:8080/q/swagger-ui/`
2. Scroll to the **General** section and find `POST /v1/register/{number}`.
3. Click "Try it out". Enter your bot's phone number exactly as written in your `.env` file (e.g., `+41796000000`).
4. If you are using a landline or VoIP number that can only receive calls, change `"use_voice": false` to `true` in the request body. Execute the request.
5. You will receive an SMS or a voice call with a 6-digit code.
6. Still in the Swagger UI, navigate to `POST /v1/register/{number}/verify/{code}`.
7. Enter your phone number and the 6-digit code to complete registration.

#### Option B: Using curl
1. Request an SMS code:
   ```bash
   curl -X POST 'http://127.0.0.1:8080/v1/register/+1234567890' \
        -H 'accept: application/json' \
        -H 'Content-Type: application/json' \
        -d '{"use_voice": false}' # set to true for voice call
   ```
2. Verify the code:
   ```bash
   curl -X POST 'http://127.0.0.1:8080/v1/register/+1234567890/verify/123456'
   ```

### 3. Running the Complete Application

Once your Signal number is registered and verified, you can launch the complete stack.

```bash
docker-compose up -d --build
```

This will build the Rust binary and start the `fitness-coach` bot daemon alongside the `signal-api`.

## Usage

Send a message on Signal to the bot phone number you registered to interact with it.

- `/status` -> Returns your current Body Battery, Sleep Score, and today's planned workouts.
- `/generate` -> Triggers the LLM to analyze your recent health/activity data, generate a new plan, and upload it to your Garmin calendar.
- `/macros <kcal> <protein>` -> Logs your nutrition for the day to the local `fitness_journal.db`. (e.g., `/macros 2500 150`)

## Architecture

- **`src/*.rs`**: The main Rust application, API server, and Signal WebSocket listener. Garmin Connect logic (authentication, data fetching, and workout construction) is implemented entirely in native Rust. 
- **`dashboard/`**: A React-based frontend dashboard configured with Next.js and `react-body-highlighter` to visualize scheduled workouts and muscle fatigue.
- **`fitness_journal.db`**: Local SQLite database storing workout logs, exercise metadata, and max progression tracking.

## Running the Dashboard

The application also includes a local dashboard to view upcoming scheduled workouts and human body graphics showing muscle fatigue.

1. Start the Rust API backend:
   ```bash
   cargo run -- --api
   ```
2. In a separate terminal, start the Next.js frontend:
   ```bash
   cd dashboard
   npm install   # Only needed once
   npm run dev
   ```
3. Open `http://localhost:3000` in your browser.

## Special Thanks

This project was built with reference to the following open-source projects:

- **[garminconnect](https://github.com/cyberjunky/python-garminconnect)** - Python Garmin Connect API wrapper which served as a valuable reference for the Garmin API endpoints.
- **[garth](https://github.com/matin/garth)** - Garmin SSO OAuth toolkit for Python, which heavily inspired our native Rust login implementation.
- **[signal-cli-rest-api](https://github.com/bbernhard/signal-cli-rest-api)** - Dockerized REST API wrapper around `signal-cli` that powers our bot's communication with the Signal network.
- **[react-body-highlighter](https://github.com/GV79/react-body-highlighter)** - React component used in the dashboard to visualize muscle fatigue on a human body graphic.
