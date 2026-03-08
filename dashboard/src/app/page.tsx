import React from 'react';
import nextDynamic from 'next/dynamic';
import Link from 'next/link';
import GenerateButton from './GenerateButton';
import AnalyzeButton from './AnalyzeButton';
import AnalyzeUpcomingButton from './AnalyzeUpcomingButton';
import ForcePullButton from './ForcePullButton';
import ReadinessRing from './ReadinessRing';
import WeeklyCalendar from './WeeklyCalendar';
import type { RecoveryHistoryEntry } from './RecoveryHistoryChart';

const RecoveryHistoryChart = nextDynamic(() => import('./RecoveryHistoryChart'), {
  loading: () => <div className="h-64 animate-pulse bg-white/5 rounded-2xl" />,
});

const MuscleMap = nextDynamic(() => import('./MuscleMap'), {
  loading: () => <div className="h-20 animate-pulse bg-white/5 rounded-2xl" />,
});

import ChatFab from './ChatFab';
export const dynamic = 'force-dynamic';

type ProgressionItem = {
  exercise_name: string;
  max_weight: number;
  reps: number;
  date: string;
  history?: { weight: number; reps: number; date: string }[];
};

type CompletedWorkout = {
  name?: string;
  type?: unknown;
  activity_type?: unknown;
  sport?: unknown;
  duration?: number;
  distance?: number;
  averageHR?: number;
};

const extractType = (val: unknown): string => {
  if (!val) return "";
  if (typeof val === 'string') return val;
  if (typeof val === 'object' && val !== null) {
    const obj = val as Record<string, unknown>;
    return String(obj.typeKey || obj.type || obj.name || "activity");
  }
  return String(val);
};

type PlannedWorkout = {
  title?: string;
  name?: string;
  date: string;
  sport?: string;
  type?: string;
  is_race?: boolean;
  primary_event?: boolean;
  duration?: number;
  distance?: number;
  description?: string;
  adaptive_details?: any;
};

async function estimateDuration(workout: PlannedWorkout): Promise<number | undefined> {
  if (workout.duration) return workout.duration;
  if (workout.adaptive_details?.estimatedDurationInSecs) {
    return Math.round(workout.adaptive_details.estimatedDurationInSecs / 60);
  }

  const isRace = workout.is_race === true || workout.type === 'race' || workout.type === 'event' || workout.type === 'primaryEvent';
  const isPrimary = workout.primary_event === true || workout.type === 'primaryEvent';
  const isRunning = (workout.sport || workout.type || "").toLowerCase().includes('run');

  if (isRace || isPrimary || isRunning) {
    return undefined;
  }

  try {
    const res = await backendFetch(`/api/predict_duration`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        title: workout.title || workout.name || "",
        sport: workout.sport || workout.type || "",
        description: workout.description || ""
      })
    });
    if (res.ok) {
      const data = await res.json();
      if (data.duration) return data.duration;
    }
  } catch (err) {
    console.error("Failed to predict duration", err);
  }

  return undefined;
}

const FITNESS_API_BASE_URL = (process.env.FITNESS_API_BASE_URL || 'http://fitness-api:3001').replace(/\/+$/, '');
const FITNESS_API_TOKEN = process.env.FITNESS_API_TOKEN || process.env.API_AUTH_TOKEN;

async function backendFetch(path: string, init: RequestInit = {}): Promise<Response> {
  const headers = new Headers(init.headers);
  if (FITNESS_API_TOKEN) {
    headers.set('x-api-token', FITNESS_API_TOKEN);
  }
  return fetch(`${FITNESS_API_BASE_URL}${path}`, {
    ...init,
    headers,
    cache: 'no-store',
  });
}

function Sparkline({ history }: { history: { weight: number, reps?: number, date: string }[] }) {
  if (!history || history.length < 2) return <div className="text-xs text-gray-600 italic">No trend</div>;

  const weights = history.map(h => h.weight);
  const minWeight = Math.min(...weights);
  const maxWeight = Math.max(...weights);
  const paddingW = maxWeight === minWeight ? 10 : (maxWeight - minWeight) * 0.2;
  const minY_W = minWeight - paddingW;
  const maxY_W = maxWeight + paddingW;

  const width = 100;
  const height = 40;

  const pointsW = history.map((h, i) => {
    const x = (i / (history.length - 1)) * width;
    const y = height - ((h.weight - minY_W) / (maxY_W - minY_W)) * height;
    return `${x},${y}`;
  }).join(' ');

  const isPositiveTrend = history[history.length - 1].weight >= history[0].weight;
  const strokeColorW = isPositiveTrend ? "rgba(52,211,153,0.8)" : "rgba(248,113,113,0.8)";
  const strokeColorR = "rgba(167,139,250,0.6)";

  const hasReps = history.some(h => h.reps !== undefined);
  let pointsR = "";
  if (hasReps) {
    const repsList = history.map(h => h.reps || 0);
    const minReps = Math.min(...repsList);
    const maxReps = Math.max(...repsList);
    const paddingR = maxReps === minReps ? 2 : (maxReps - minReps) * 0.2;
    const minY_R = minReps - paddingR;
    const maxY_R = maxReps + paddingR;

    pointsR = history.map((h, i) => {
      const r = h.reps || 0;
      const x = (i / (history.length - 1)) * width;
      const y = height - ((r - minY_R) / (maxY_R - minY_R)) * height;
      return `${x},${y}`;
    }).join(' ');
  }

  return (
    <svg viewBox={`0 0 ${width} ${height}`} className="w-full h-full overflow-visible">
      {hasReps && (
        <polyline
          fill="none"
          stroke={strokeColorR}
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeDasharray="3 3"
          points={pointsR}
          className="opacity-60 group-hover:opacity-100 transition-opacity"
        />
      )}

      <polyline
        fill="none"
        stroke={strokeColorW}
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
        points={pointsW}
        style={{ filter: `drop-shadow(0px 2px 6px ${strokeColorW})` }}
        className="opacity-75 group-hover:opacity-100 transition-opacity"
      />
      <circle cx="0" cy={height - ((history[0].weight - minY_W) / (maxY_W - minY_W)) * height} r="2.5" fill={strokeColorW} className="opacity-75 group-hover:opacity-100 transition-opacity" />
      <circle cx={width} cy={height - ((history[history.length - 1].weight - minY_W) / (maxY_W - minY_W)) * height} r="2.5" fill={strokeColorW} className="opacity-75 group-hover:opacity-100 transition-opacity" />
    </svg>
  );
}

type WeeklyDelta = {
  exercise_name: string;
  this_week_weight: number;
  this_week_reps: number;
  last_week_weight: number;
  last_week_reps: number;
};

async function fetchWeeklyDeltas(): Promise<WeeklyDelta[]> {
  try {
    const res = await backendFetch('/api/progression/deltas');
    if (!res.ok) return [];
    return await res.json();
  } catch {
    return [];
  }
}

async function fetchProgression(): Promise<ProgressionItem[]> {
  try {
    const res = await backendFetch('/api/progression');
    if (!res.ok) {
      return [];
    }
    return await res.json();
  } catch (err) {
    console.error("Fetch failed. Is the Rust API running on port 3001?", err);
    return [];
  }
}

type RecoveryItem = {
  body_battery: number | null;
  sleep_score: number | null;
  training_readiness: number | null;
  hrv_status: string | null;
  hrv_weekly_avg: number | null;
  hrv_last_night_avg: number | null;
  rhr_trend: number[];
};

async function fetchRecovery(): Promise<RecoveryItem> {
  try {
    const res = await backendFetch('/api/recovery');
    if (!res.ok) {
      return { body_battery: null, sleep_score: null, training_readiness: null, hrv_status: null, hrv_weekly_avg: null, hrv_last_night_avg: null, rhr_trend: [] };
    }
    return await res.json();
  } catch (err) {
    console.error("Fetch failed for recovery metrics.", err);
    return { body_battery: null, sleep_score: null, training_readiness: null, hrv_status: null, hrv_weekly_avg: null, hrv_last_night_avg: null, rhr_trend: [] };
  }
}

async function fetchRecoveryHistory(): Promise<RecoveryHistoryEntry[]> {
  try {
    const res = await backendFetch('/api/recovery/history');
    if (!res.ok) {
      return [];
    }
    return await res.json();
  } catch (err) {
    console.error("Fetch failed for recovery history.", err);
    return [];
  }
}

type TodayWorkoutsResponse = {
  done: CompletedWorkout[];
  planned: PlannedWorkout[];
};

async function fetchTodayWorkouts(): Promise<TodayWorkoutsResponse> {
  try {
    const res = await backendFetch('/api/workouts/today');
    if (!res.ok) {
      return { done: [], planned: [] };
    }
    return await res.json();
  } catch (err) {
    console.error("Fetch failed for today workouts.", err);
    return { done: [], planned: [] };
  }
}

async function fetchUpcomingWorkouts(): Promise<PlannedWorkout[]> {
  try {
    const res = await backendFetch('/api/workouts/upcoming');
    if (!res.ok) {
      return [];
    }
    return await res.json();
  } catch (err) {
    console.error("Fetch failed for upcoming workouts.", err);
    return [];
  }
}

type WeekActivity = {
  name?: string;
  type?: unknown;
  activity_type?: unknown;
  sport?: unknown;
  startTimeLocal?: string;
  duration?: number;
  distance?: number;
  averageHR?: number;
  maxHR?: number;
  calories?: number;
  averageSpeed?: number;
  elevationGain?: number;
  avgPower?: number;
  sets?: unknown;
};

async function fetchWeekActivities(): Promise<WeekActivity[]> {
  try {
    const res = await backendFetch('/api/activities/week');
    if (!res.ok) {
      return [];
    }
    return await res.json();
  } catch (err) {
    console.error("Fetch failed for week activities.", err);
    return [];
  }
}

export default async function Dashboard() {
  const [data, recovery, recoveryHistory, todayWorkouts, upcomingWorkouts, weeklyDeltas, weekActivities] = await Promise.all([
    fetchProgression(),
    fetchRecovery(),
    fetchRecoveryHistory(),
    fetchTodayWorkouts(),
    fetchUpcomingWorkouts(),
    fetchWeeklyDeltas(),
    fetchWeekActivities(),
  ]);

  const isWorkoutDone = (planned: PlannedWorkout) => {
    const pTitle = extractType(planned.title || planned.name).toLowerCase();
    const pSport = extractType(planned.sport || planned.type).toLowerCase().replace(/_|\s/g, '');

    return todayWorkouts.done.some((done: CompletedWorkout) => {
      const dName = extractType(done.name).toLowerCase();
      const dType = extractType(done.type || done.activity_type || done.sport).toLowerCase().replace(/_|\s/g, '');

      if (pTitle && dName && (dName.includes(pTitle) || pTitle.includes(dName))) {
        return true;
      }
      if (pSport && dType && (dType.includes(pSport) || pSport.includes(dType))) {
        return true;
      }
      return false;
    });
  };

  const activePlannedWorkouts = todayWorkouts.planned.filter((w: PlannedWorkout) => !isWorkoutDone(w));

  const activePlannedWorkoutsWithPrediction = await Promise.all(
    activePlannedWorkouts.map(async (workout: PlannedWorkout) => {
      const estDur = await estimateDuration(workout);
      return { ...workout, estDur };
    })
  );

  const todayDates = new Set(todayWorkouts.planned.map((w: PlannedWorkout) => w.date));
  const activeUpcomingWorkoutsRaw = upcomingWorkouts.filter((w: PlannedWorkout) => {
    if (todayDates.has(w.date)) {
      return !isWorkoutDone(w);
    }
    return true;
  });

  const activeUpcomingWorkoutsWithPrediction = await Promise.all(
    activeUpcomingWorkoutsRaw.map(async (workout: PlannedWorkout) => {
      const estDur = await estimateDuration(workout);
      return { ...workout, estDur };
    })
  );

  return (
    <main className="min-h-screen p-4 md:p-8 lg:p-12 selection:bg-red-500 selection:text-white pb-24">
      <div className="max-w-6xl mx-auto space-y-2">

        {/* ─── Header ─── */}
        <header className="flex items-center justify-between stagger-item" style={{ '--stagger-index': 0 } as React.CSSProperties}>
          <div>
            <h1 className="text-2xl md:text-3xl font-extrabold tracking-tight bg-clip-text text-transparent bg-gradient-to-r from-red-400 via-orange-400 to-amber-400">
              Fitness Journal
            </h1>
          </div>
          <div className="flex items-center gap-2">
            <ForcePullButton />
            <Link href="/settings" className="px-3 py-2 glass-panel text-white rounded-xl hover:bg-white/10 transition-all flex items-center gap-2 h-[38px] text-xs font-medium">
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"></path><circle cx="12" cy="12" r="3"></circle></svg>
              Settings
            </Link>
          </div>
        </header>

        {/* ─── Bento Grid: Hero + Metrics ─── */}
        <div className="bento-grid">

          {/* Hero: Readiness Ring */}
          <div className="stagger-item h-full" style={{ '--stagger-index': 1 } as React.CSSProperties}>
            <ReadinessRing value={recovery.training_readiness} />
          </div>

          {/* Body Battery */}
          <div className="glass-panel hover-lift p-4 flex flex-col justify-center group relative overflow-hidden stagger-item h-full" style={{ '--stagger-index': 2 } as React.CSSProperties}>
            <div className="ambient-glow-sm bg-red-500 -top-6 -right-6 opacity-0 group-hover:opacity-20 transition-opacity duration-500" />
            <h3 className="text-gray-400 font-medium tracking-wide text-xs uppercase">Body Battery</h3>
            <div className="mt-2 flex items-center justify-between">
              <div className="flex items-end gap-2 text-white group-hover:text-red-400 transition-colors duration-300">
                <span className="text-4xl font-bold tracking-tighter">{recovery.body_battery ?? '--'}</span>
                <span className="text-gray-500 mb-1 text-xs">/100</span>
              </div>
              {recoveryHistory.length > 1 && (
                <div className="w-16 h-8">
                  <Sparkline history={recoveryHistory.filter(e => e.body_battery != null).map(e => ({ weight: e.body_battery!, date: e.date }))} />
                </div>
              )}
            </div>
          </div>

          {/* Sleep Score */}
          <div className="glass-panel hover-lift p-4 flex flex-col justify-center group relative overflow-hidden stagger-item h-full" style={{ '--stagger-index': 3 } as React.CSSProperties}>
            <div className="ambient-glow-sm bg-indigo-500 -top-6 -right-6 opacity-0 group-hover:opacity-20 transition-opacity duration-500" />
            <h3 className="text-gray-400 font-medium tracking-wide text-xs uppercase">Sleep Score</h3>
            <div className="mt-2 flex items-center justify-between">
              <div className="flex items-end gap-2 text-white group-hover:text-indigo-400 transition-colors duration-300">
                <span className="text-4xl font-bold tracking-tighter">{recovery.sleep_score ?? '--'}</span>
                <span className="text-gray-500 mb-1 text-xs">/100</span>
              </div>
              {recoveryHistory.length > 1 && (
                <div className="w-16 h-8">
                  <Sparkline history={recoveryHistory.filter(e => e.sleep_score != null).map(e => ({ weight: e.sleep_score!, date: e.date }))} />
                </div>
              )}
            </div>
          </div>

          {/* HRV */}
          <div className="glass-panel hover-lift p-4 flex flex-col justify-center group relative overflow-hidden stagger-item h-full" style={{ '--stagger-index': 4 } as React.CSSProperties}>
            <div className="ambient-glow-sm bg-purple-500 -top-6 -right-6 opacity-0 group-hover:opacity-20 transition-opacity duration-500" />
            <h3 className="text-gray-400 font-medium tracking-wide text-xs uppercase">HRV</h3>
            <div className="mt-2 flex items-center justify-between">
              <div className="flex flex-col gap-0.5">
                <div className="text-3xl font-bold tracking-tight text-purple-400">
                  {recovery.hrv_last_night_avg ?? '--'} <span className="text-sm font-normal text-gray-500">ms</span>
                </div>
                <div className="text-xs text-gray-500">
                  <span>{recovery.hrv_status ?? '--'}</span>
                  {' · '}
                  <span>7d: {recovery.hrv_weekly_avg ?? '--'} ms</span>
                </div>
              </div>
              {recoveryHistory.length > 1 && (
                <div className="w-16 h-8">
                  <Sparkline history={recoveryHistory.filter(e => e.hrv_last_night_avg != null).map(e => ({ weight: e.hrv_last_night_avg!, date: e.date }))} />
                </div>
              )}
            </div>
          </div>

          {/* Resting HR */}
          <div className="glass-panel hover-lift p-4 flex flex-col justify-center group relative overflow-hidden stagger-item h-full" style={{ '--stagger-index': 5 } as React.CSSProperties}>
            <div className="ambient-glow-sm bg-rose-500 -top-6 -right-6 opacity-0 group-hover:opacity-20 transition-opacity duration-500" />
            <h3 className="text-gray-400 font-medium tracking-wide text-xs uppercase">Resting HR</h3>
            <div className="mt-2 flex items-center justify-between">
              <div className="text-3xl font-bold tracking-tight text-rose-400">
                {recovery.rhr_trend && recovery.rhr_trend.length > 0 ? recovery.rhr_trend[recovery.rhr_trend.length - 1] : '--'} <span className="text-sm font-normal text-gray-500">bpm</span>
              </div>
              {recovery.rhr_trend && recovery.rhr_trend.length > 0 && (
                <div className="w-16 h-8">
                  <Sparkline history={recovery.rhr_trend.map((val, idx) => ({ weight: val, date: String(idx) }))} />
                </div>
              )}
            </div>
          </div>
        </div>

        {/* ─── Upcoming Events ─── */}
        {(() => {
          const isEvent = (w: PlannedWorkout) => w.is_race || w.primary_event || w.type === 'race' || w.type === 'event' || w.type === 'primaryEvent';
          const upcomingEvents = activeUpcomingWorkoutsWithPrediction.filter(w => isEvent(w));

          return upcomingEvents.length > 0 ? (
            <section className="glass-panel p-3 section-reveal">
              <div className="flex items-center justify-between mb-2">
                <h2 className="text-sm font-bold tracking-tight text-amber-400 section-header">Upcoming Events</h2>
              </div>
              <div className="space-y-0.5">
                {upcomingEvents.map((workout, idx) => {
                  const isPrimary = workout.primary_event === true || workout.type === 'primaryEvent';
                  const eventDate = new Date(workout.date);
                  const today = new Date();
                  today.setHours(0, 0, 0, 0);
                  eventDate.setHours(0, 0, 0, 0);
                  const daysUntil = Math.round((eventDate.getTime() - today.getTime()) / (1000 * 60 * 60 * 24));
                  const distanceKm = (workout.distance || workout.adaptive_details?.estimatedDistanceInMeters)
                    ? ((workout.distance || workout.adaptive_details?.estimatedDistanceInMeters) / 1000).toFixed(1) : null;
                  const daysLabel = daysUntil === 0 ? 'Today' : daysUntil === 1 ? 'Tomorrow' : `${daysUntil}d`;
                  const dateLabel = new Date(workout.date).toLocaleDateString(undefined, { weekday: 'short', month: 'short', day: 'numeric' });

                  const eventName = workout.adaptive_details?.workoutName || workout.title || workout.description || "Event";

                  return (
                    <div key={idx} className={`flex items-center gap-2 py-1.5 px-2.5 rounded-lg transition-all hover:bg-white/[0.03] flex-wrap ${isPrimary ? 'bg-amber-500/[0.04] border border-amber-500/20' : 'border border-transparent'}`}>
                      {isPrimary && <span className="text-[10px]">⭐</span>}
                      <span className={`text-xs font-medium truncate ${isPrimary ? 'text-amber-400' : 'text-slate-200'}`}>
                        {eventName}
                      </span>
                      {distanceKm && <span className="text-[10px] text-gray-500">{distanceKm} km</span>}
                      <span className={`text-[10px] font-semibold ${daysUntil <= 7 ? 'text-amber-400' : 'text-gray-500'}`}>{daysLabel}</span>
                      <span className="text-[10px] text-gray-600 ml-auto">{dateLabel}</span>
                      {isPrimary && <AnalyzeUpcomingButton workout={workout} />}
                    </div>
                  );
                })}
              </div>
            </section>
          ) : null;
        })()}

        {/* ─── Weekly Calendar Strip ─── */}
        <WeeklyCalendar
          upcomingWorkouts={activeUpcomingWorkoutsWithPrediction}
          todayPlanned={activePlannedWorkoutsWithPrediction}
          completedCount={todayWorkouts.done.length}
          weekActivities={weekActivities}
        />

        {/* ─── Two-Column: Recovery Chart + Workouts ─── */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-3">

          {/* Recovery Trends */}
          {recoveryHistory && recoveryHistory.length > 0 && (
            <section className="glass-panel p-4 relative overflow-hidden section-reveal">
              <div className="ambient-glow bg-rose-500 -top-16 -right-16 opacity-10" style={{ width: '180px', height: '180px' }} />
              <h2 className="text-base font-bold tracking-tight text-rose-400 section-header mb-3">Recovery Trends</h2>
              <RecoveryHistoryChart data={recoveryHistory} />
            </section>
          )}

          {/* Today's Workouts (Planned + Completed) */}
          <section className="glass-panel p-4 relative overflow-hidden section-reveal">
            <div className="flex items-center justify-between mb-3">
              <h2 className="text-base font-bold tracking-tight text-emerald-400 section-header">Today&apos;s Workouts</h2>
              <GenerateButton />
            </div>

            {/* Planned */}
            {activePlannedWorkoutsWithPrediction.length > 0 && (
              <div className="mb-4">
                <h4 className="text-xs text-gray-500 uppercase tracking-wider mb-3 font-medium">Planned</h4>
                <div className="space-y-2">
                  {activePlannedWorkoutsWithPrediction.map((workout, idx) => {
                    const estDur = workout.estDur;
                    const isEstimated = !workout.duration && !!estDur;
                    const displayDur = workout.duration || estDur;
                    return (
                      <div key={idx} className="bg-white/[0.02] hover:bg-white/[0.05] p-3 rounded-xl border border-indigo-500/15 transition-all group">
                        <div className="flex justify-between items-center">
                          <span className="text-sm text-white font-medium">{workout.adaptive_details?.workoutName || workout.title || workout.description || "Training"}</span>
                          <span className="text-[10px] px-2 py-0.5 bg-indigo-500/15 text-indigo-300 rounded-full border border-indigo-500/20">{workout.type || workout.sport || "Workout"}</span>
                        </div>
                        {(workout.adaptive_details?.description) && (
                          <div className="mt-1 text-xs text-gray-400 italic">
                            {workout.adaptive_details.description}
                          </div>
                        )}
                        {(displayDur || workout.distance || workout.adaptive_details?.estimatedDistanceInMeters) && (
                          <div className="mt-1.5 flex items-center gap-3 text-xs text-indigo-200">
                            {displayDur ? (
                              <span className={isEstimated ? "text-indigo-400/80" : ""}>
                                {isEstimated && <span title="Predicted">✨ ~</span>}{displayDur.toFixed(0)} min
                              </span>
                            ) : null}
                            {(workout.distance || workout.adaptive_details?.estimatedDistanceInMeters) ? <span>{((workout.distance || workout.adaptive_details?.estimatedDistanceInMeters) / 1000).toFixed(1)} km</span> : null}
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              </div>
            )}

            {/* Completed */}
            {todayWorkouts.done.length > 0 ? (
              <div>
                <h4 className="text-xs text-gray-500 uppercase tracking-wider mb-3 font-medium">Completed</h4>
                <div className="space-y-2">
                  {todayWorkouts.done.map((workout: CompletedWorkout, idx: number) => (
                    <div key={idx} className="bg-white/[0.02] hover:bg-white/[0.05] p-3 rounded-xl border border-emerald-500/15 transition-all group">
                      <div className="flex justify-between items-center">
                        <span className="text-sm text-white font-medium">{workout.name}</span>
                        <span className="text-[10px] px-2 py-0.5 bg-emerald-500/15 text-emerald-300 rounded-full border border-emerald-500/20">{extractType(workout.type || workout.activity_type) || "Activity"}</span>
                      </div>
                      <div className="mt-1.5 flex items-center gap-3 text-xs text-gray-400">
                        {!!workout.duration && <span>{(workout.duration / 60).toFixed(0)} min</span>}
                        {!!workout.distance && <span>{(workout.distance / 1000).toFixed(1)} km</span>}
                        {!!workout.averageHR && <span>{Math.round(workout.averageHR)} bpm</span>}
                      </div>
                      <AnalyzeButton workout={workout} />
                    </div>
                  ))}
                </div>
              </div>
            ) : activePlannedWorkoutsWithPrediction.length === 0 ? (
              <div className="py-8 text-center text-gray-500 text-sm">
                {todayWorkouts.planned.length > 0 ? "All workouts completed! ✅" : "No workouts planned or completed today."}
              </div>
            ) : null}
          </section>
        </div>

        {/* ─── Strength Progression ─── */}
        <section className="space-y-3 section-reveal">
          <h2 className="text-base font-bold tracking-tight section-header">Strength Progress</h2>

          {/* Week-over-Week Deltas */}
          {weeklyDeltas.length > 0 && (
            <div>
              <h4 className="text-xs text-gray-500 uppercase tracking-wider mb-3 font-medium">This Week vs Last</h4>
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-3">
                {weeklyDeltas.map((delta, idx) => {
                  const weightDiff = delta.this_week_weight - delta.last_week_weight;
                  const repsDiff = delta.this_week_reps - delta.last_week_reps;
                  const isUp = weightDiff > 0 || (weightDiff === 0 && repsDiff > 0);
                  const isDown = weightDiff < 0 || (weightDiff === 0 && repsDiff < 0);

                  return (
                    <div key={idx} className="glass-panel hover-lift p-4 group relative overflow-hidden stagger-item" style={{ '--stagger-index': idx } as React.CSSProperties}>
                      <h4 className="text-gray-400 text-xs font-medium tracking-wider truncate" title={delta.exercise_name}>
                        {delta.exercise_name}
                      </h4>
                      <div className="mt-2 flex items-center justify-between">
                        <div className="flex items-baseline gap-1.5">
                          <span className="text-2xl font-extrabold tracking-tight text-white">{delta.this_week_weight.toFixed(1)}</span>
                          <span className="text-gray-500 text-xs">kg × {delta.this_week_reps}</span>
                        </div>
                        <span className={`text-lg font-bold ${isUp ? 'text-emerald-400' : isDown ? 'text-red-400' : 'text-gray-500'}`} style={{ filter: isUp ? 'drop-shadow(0 0 6px rgba(52,211,153,0.4))' : isDown ? 'drop-shadow(0 0 6px rgba(248,113,113,0.4))' : 'none' }}>
                          {isUp ? '↑' : isDown ? '↓' : '='}
                        </span>
                      </div>
                      {delta.last_week_weight > 0 && (
                        <div className="mt-1.5 pt-1.5 border-t border-white/5 text-[11px] text-gray-500">
                          Last: {delta.last_week_weight.toFixed(1)}kg × {delta.last_week_reps}
                          {weightDiff !== 0 && (
                            <span className={`ml-1.5 ${weightDiff > 0 ? 'text-emerald-400' : 'text-red-400'}`}>
                              ({weightDiff > 0 ? '+' : ''}{weightDiff.toFixed(1)}kg)
                            </span>
                          )}
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {/* All-Time Personal Bests */}
          <div>
            <h4 className="text-xs text-gray-500 uppercase tracking-wider mb-3 font-medium">Personal Bests</h4>
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-3">
              {data.length === 0 ? (
                <div className="col-span-full py-12 text-center text-gray-400 glass-panel">
                  <div className="text-base mb-1 font-medium">No progression data found.</div>
                  <div className="text-xs">Ensure the API is running on <code className="text-red-500 bg-red-500/10 px-1.5 py-0.5 rounded">port 3001</code>.</div>
                </div>
              ) : data.map((item, idx) => (
                <div key={idx} className="glass-panel hover-lift p-4 group cursor-default relative overflow-hidden stagger-item" style={{ '--stagger-index': idx } as React.CSSProperties}>
                  <h4 className="text-gray-400 text-xs font-medium tracking-wider truncate" title={item.exercise_name}>
                    {item.exercise_name}
                  </h4>
                  <div className="mt-2 flex items-center justify-between">
                    <div className="flex items-baseline gap-1.5">
                      <span className="text-3xl font-extrabold tracking-tight text-white group-hover:text-red-100 transition-colors duration-300">{item.max_weight.toFixed(1)}</span>
                      <span className="text-gray-500 text-xs font-medium">kg</span>
                    </div>
                    {item.history && item.history.length > 0 && (
                      <div className="w-16 h-8">
                        <Sparkline history={item.history} />
                      </div>
                    )}
                  </div>
                  <div className="mt-2 pt-2 border-t border-white/5 flex items-center justify-between text-xs">
                    <span className="text-gray-400">
                      <span className="text-emerald-400 font-bold">{item.reps}</span> reps
                    </span>
                    <span className="text-gray-600 truncate max-w-[100px]" title={item.date}>{item.date}</span>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </section>

        {/* ─── Muscle Map (Collapsible) ─── */}
        <section className="section-reveal">
          <MuscleMap />
        </section>
      </div>

      {/* ─── Chat FAB ─── */}
      <ChatFab />
    </main>
  );
}
