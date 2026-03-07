import React from 'react';
import nextDynamic from 'next/dynamic';
import Link from 'next/link';
import GenerateButton from './GenerateButton';
import Chat from './Chat';
import AnalyzeButton from './AnalyzeButton';
import AnalyzeUpcomingButton from './AnalyzeUpcomingButton';
import ForcePullButton from './ForcePullButton';
import type { RecoveryHistoryEntry } from './RecoveryHistoryChart';

const RecoveryHistoryChart = nextDynamic(() => import('./RecoveryHistoryChart'), {
  loading: () => <div className="h-64 animate-pulse bg-white/5 rounded-2xl" />,
});

const MuscleMap = nextDynamic(() => import('./MuscleMap'), {
  loading: () => <div className="h-96 animate-pulse bg-white/5 rounded-2xl" />,
});

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
};

async function estimateDuration(workout: PlannedWorkout): Promise<number | undefined> {
  if (workout.duration) return workout.duration;

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

export default async function Dashboard() {
  const [data, recovery, recoveryHistory, todayWorkouts, upcomingWorkouts, weeklyDeltas] = await Promise.all([
    fetchProgression(),
    fetchRecovery(),
    fetchRecoveryHistory(),
    fetchTodayWorkouts(),
    fetchUpcomingWorkouts(),
    fetchWeeklyDeltas(),
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
    <main className="min-h-screen p-6 md:p-16 lg:p-24 selection:bg-red-500 selection:text-white pb-32">
      <div className="max-w-6xl mx-auto space-y-16">

        {/* ─── Header ─── */}
        <header className="flex flex-col md:flex-row md:items-start md:justify-between space-y-4 md:space-y-0 stagger-item" style={{ '--stagger-index': 0 } as React.CSSProperties}>
          <div className="space-y-4 relative">
            <div className="ambient-glow-sm bg-red-500" style={{ top: '-20px', left: '-30px' }} />
            <h1 className="text-4xl md:text-6xl font-extrabold tracking-tight bg-clip-text text-transparent bg-gradient-to-r from-red-400 via-orange-400 to-amber-400" style={{ textShadow: '0 0 80px rgba(248, 113, 113, 0.3)' }}>
              Fitness Journal
            </h1>
            <p className="text-gray-400 text-lg md:text-xl max-w-2xl">
              Live AI Coaching Dashboard and Garmin Connect Integration.
            </p>
          </div>
          <div className="flex items-center gap-3">
            <ForcePullButton />
            <Link href="/settings" className="px-4 py-2.5 glass-panel text-white rounded-xl hover:bg-white/10 transition-all flex items-center gap-2 h-[42px] text-sm font-medium">
              <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"></path><circle cx="12" cy="12" r="3"></circle></svg>
              Configuration
            </Link>
          </div>
        </header>

        {/* ─── Recovery Metrics ─── */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-6 gap-5">
          <div className="glass-panel hover-lift p-6 flex flex-col justify-between group relative overflow-hidden stagger-item" style={{ '--stagger-index': 1 } as React.CSSProperties}>
            <div className="ambient-glow-sm bg-red-500 -top-6 -right-6 opacity-0 group-hover:opacity-20 transition-opacity duration-500" />
            <h3 className="text-gray-400 font-medium tracking-wide text-xs uppercase">Body Battery</h3>
            <div className="mt-4 flex items-end gap-2 text-white group-hover:text-red-400 transition-colors duration-300">
              <span className="text-5xl font-bold tracking-tighter">{recovery.body_battery ?? '--'}</span>
              <span className="text-gray-500 mb-1 text-sm">/100</span>
            </div>
            <div className="mt-3 text-[10px] text-gray-600 uppercase tracking-widest">Garmin Connect</div>
          </div>

          <div className="glass-panel hover-lift p-6 flex flex-col justify-between group relative overflow-hidden stagger-item" style={{ '--stagger-index': 2 } as React.CSSProperties}>
            <div className="ambient-glow-sm bg-indigo-500 -top-6 -right-6 opacity-0 group-hover:opacity-20 transition-opacity duration-500" />
            <h3 className="text-gray-400 font-medium tracking-wide text-xs uppercase">Sleep Score</h3>
            <div className="mt-4 flex items-end gap-2 text-white group-hover:text-indigo-400 transition-colors duration-300">
              <span className="text-5xl font-bold tracking-tighter">{recovery.sleep_score ?? '--'}</span>
              <span className="text-gray-500 mb-1 text-sm">/100</span>
            </div>
            <div className="mt-3 text-[10px] text-gray-600 uppercase tracking-widest">Garmin Connect</div>
          </div>

          <div className="glass-panel hover-lift p-6 flex flex-col justify-between group relative overflow-hidden stagger-item" style={{ '--stagger-index': 3 } as React.CSSProperties}>
            <div className="ambient-glow-sm bg-emerald-500 -top-6 -right-6 opacity-0 group-hover:opacity-20 transition-opacity duration-500" />
            <h3 className="text-gray-400 font-medium tracking-wide text-xs uppercase">Training Readiness</h3>
            <div className="mt-4 flex items-end gap-2 text-white group-hover:text-emerald-400 transition-colors duration-300">
              <span className="text-5xl font-bold tracking-tighter">{recovery.training_readiness ?? '--'}</span>
              <span className="text-gray-500 mb-1 text-sm">/100</span>
            </div>
            <div className="mt-3 text-[10px] text-gray-600 uppercase tracking-widest">Garmin Connect</div>
          </div>

          <div className="glass-panel hover-lift p-6 flex flex-col justify-between group relative overflow-hidden stagger-item" style={{ '--stagger-index': 4 } as React.CSSProperties}>
            <div className="ambient-glow-sm bg-purple-500 -top-6 -right-6 opacity-0 group-hover:opacity-20 transition-opacity duration-500" />
            <h3 className="text-gray-400 font-medium tracking-wide text-xs uppercase">HRV Status</h3>
            <div className="mt-4 flex flex-col gap-1 text-white group-hover:text-purple-400 transition-colors duration-300">
              <div className="text-4xl font-bold tracking-tight py-1 text-purple-400">
                {recovery.hrv_last_night_avg ?? '--'} <span className="text-base font-normal text-gray-500">ms</span>
              </div>
              <div className="text-sm text-gray-400 flex flex-col gap-1.5 mt-1">
                <div className="flex justify-between">
                  <span className="text-gray-500">Status</span>
                  <span className="text-white font-medium uppercase text-xs">{recovery.hrv_status ?? '--'}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-gray-500">7-Day Avg</span>
                  <span className="text-white font-medium text-xs">{recovery.hrv_weekly_avg ?? '--'} ms</span>
                </div>
              </div>
            </div>
            <div className="mt-3 text-[10px] text-gray-600 uppercase tracking-widest pt-2 border-t border-white/5">Garmin Connect</div>
          </div>

          <div className="glass-panel hover-lift p-6 flex flex-col justify-between group relative overflow-hidden stagger-item" style={{ '--stagger-index': 5 } as React.CSSProperties}>
            <div className="ambient-glow-sm bg-rose-500 -top-6 -right-6 opacity-0 group-hover:opacity-20 transition-opacity duration-500" />
            <h3 className="text-gray-400 font-medium tracking-wide text-xs uppercase">Resting HR</h3>
            <div className="mt-4 flex flex-col gap-1 text-white group-hover:text-rose-400 transition-colors duration-300">
              <div className="text-4xl font-bold tracking-tight py-1 text-rose-400">
                {recovery.rhr_trend && recovery.rhr_trend.length > 0 ? recovery.rhr_trend[recovery.rhr_trend.length - 1] : '--'} <span className="text-base font-normal text-gray-500">bpm</span>
              </div>
              <div className="h-10 w-full mt-2">
                {recovery.rhr_trend && recovery.rhr_trend.length > 0 && (
                  <Sparkline history={recovery.rhr_trend.map((val, idx) => ({ weight: val, date: String(idx) }))} />
                )}
              </div>
            </div>
            <div className="mt-3 text-[10px] text-gray-600 uppercase tracking-widest pt-2 border-t border-white/5">Garmin Connect</div>
          </div>

          <div className="h-full stagger-item" style={{ '--stagger-index': 6 } as React.CSSProperties}>
            <GenerateButton />
          </div>
        </div>

        {/* ─── Recovery History Chart ─── */}
        {recoveryHistory && recoveryHistory.length > 0 && (
          <section className="space-y-6 section-reveal">
            <div className="flex items-center justify-between">
              <h2 className="text-2xl font-bold tracking-tight text-rose-400 section-header">Recovery Trends</h2>
            </div>
            <div className="glass-panel-elevated p-6 border border-rose-500/15 relative overflow-hidden">
              <div className="ambient-glow bg-rose-500 -top-20 -right-20 opacity-10" style={{ width: '200px', height: '200px' }} />
              <RecoveryHistoryChart data={recoveryHistory} />
            </div>
          </section>
        )}

        {/* ─── Today's Planned Workouts ─── */}
        <section className="space-y-6 section-reveal">
          <div className="flex items-center justify-between">
            <h2 className="text-2xl font-bold tracking-tight text-indigo-400 section-header">Planned Today</h2>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {activePlannedWorkouts.length === 0 ? (
              <div className="col-span-full py-10 text-center text-gray-500 glass-panel border border-dashed border-gray-700/50">
                <div className="text-lg">
                  {todayWorkouts.planned.length > 0 ? "All planned workouts completed for today! ✅" : "No workouts planned for today."}
                </div>
              </div>
            ) : activePlannedWorkoutsWithPrediction.map((workout, idx: number) => {
              const estDur = workout.estDur;
              const isEstimated = !workout.duration && !!estDur;
              const displayDur = workout.duration || estDur;

              return (
                <div key={idx} className="glass-panel hover-lift p-5 group relative border border-indigo-500/15 overflow-hidden stagger-item" style={{ '--stagger-index': idx } as React.CSSProperties}>
                  <div className="absolute inset-0 bg-gradient-to-br from-indigo-500/8 to-transparent opacity-0 group-hover:opacity-100 transition-opacity duration-500 pointer-events-none" />
                  <div className="flex justify-between items-start mb-2">
                    <h4 className="text-white font-medium">{workout.title || workout.description || "Training"}</h4>
                    <span className="text-xs px-2.5 py-1 bg-indigo-500/15 text-indigo-300 rounded-full border border-indigo-500/20">{workout.type || workout.sport || "Workout"}</span>
                  </div>
                  {(displayDur || workout.distance) ? (
                    <div className="mt-4 flex items-center gap-4 text-sm text-gray-400">
                      {displayDur ? (
                        <span className={isEstimated ? "text-indigo-400/80" : ""}>
                          {isEstimated && <span title="Predicted duration">✨ ~</span>}{displayDur.toFixed(0)} min
                        </span>
                      ) : null}
                      {workout.distance ? <span>{workout.distance.toFixed(1)} km</span> : null}
                    </div>
                  ) : null}
                </div>
              );
            })}
          </div>
        </section>

        {/* ─── Upcoming Workouts ─── */}
        <section className="space-y-6 section-reveal">
          <div className="flex items-center justify-between">
            <h2 className="text-2xl font-bold tracking-tight text-purple-400 section-header">Upcoming Schedule</h2>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {activeUpcomingWorkoutsWithPrediction.length === 0 ? (
              <div className="col-span-full py-10 text-center text-gray-500 glass-panel border border-dashed border-gray-700/50">
                <div className="text-lg">No upcoming workouts planned.</div>
              </div>
            ) : activeUpcomingWorkoutsWithPrediction.map((workout, idx: number) => {
              const isRace = workout.is_race === true || workout.type === 'race' || workout.type === 'event' || workout.type === 'primaryEvent';
              const isPrimary = workout.primary_event === true || workout.type === 'primaryEvent';
              const estDur = workout.estDur;
              const isEstimated = !workout.duration && !!estDur;
              const displayDur = workout.duration || estDur;

              let borderClass = "border-purple-500/15";
              let bgHoverClass = "from-purple-500/8";
              let badgeBg = "bg-purple-500/15 text-purple-300 border border-purple-500/20";

              if (isPrimary) {
                borderClass = "border-amber-500/40 shadow-[0_0_20px_rgba(245,158,11,0.15)]";
                bgHoverClass = "from-amber-500/15";
                badgeBg = "bg-amber-500/15 text-amber-300 font-bold border border-amber-500/40";
              } else if (isRace) {
                borderClass = "border-slate-300/30 shadow-[0_0_12px_rgba(203,213,225,0.08)]";
                bgHoverClass = "from-slate-400/15";
                badgeBg = "bg-slate-400/15 text-slate-200 font-medium border border-slate-400/25";
              }

              return (
                <div key={idx} className={`glass-panel hover-lift p-5 group relative overflow-hidden transition-all border stagger-item ${borderClass}`} style={{ '--stagger-index': idx } as React.CSSProperties}>
                  <div className={`absolute inset-0 bg-gradient-to-br ${bgHoverClass} to-transparent opacity-0 group-hover:opacity-100 transition-opacity duration-500 pointer-events-none`} />

                  {isPrimary && (
                    <div className="absolute -top-3 -right-3">
                      <span className="flex h-6 w-6 relative">
                        <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-amber-400 opacity-20"></span>
                        <span className="relative inline-flex rounded-full h-6 w-6 bg-amber-500 items-center justify-center text-xs">⭐</span>
                      </span>
                    </div>
                  )}

                  <div className="flex justify-between items-start mb-2">
                    <h4 className={`font-medium ${isPrimary ? 'text-amber-400 text-lg' : (isRace ? 'text-slate-200' : 'text-white')}`}>
                      {workout.title || workout.description || "Training"}
                    </h4>
                    <span className={`text-xs px-2.5 py-1 rounded-full whitespace-nowrap ml-2 ${badgeBg}`}>
                      {new Date(workout.date).toLocaleDateString(undefined, { weekday: 'short', month: 'short', day: 'numeric' })}
                    </span>
                  </div>
                  {(displayDur || workout.distance) ? (
                    <div className="mt-4 flex items-center gap-4 text-sm text-gray-400">
                      {displayDur ? (
                        <span className={isEstimated ? "text-purple-400/80" : ""}>
                          {isEstimated && <span title="Predicted duration">✨ ~</span>}{displayDur.toFixed(0)} min
                        </span>
                      ) : null}
                      {workout.distance ? <span>{workout.distance.toFixed(1)} km</span> : null}
                    </div>
                  ) : null}
                  <div className="mt-2 text-xs flex items-center justify-between">
                    <span className={isPrimary ? 'text-amber-500/80 font-medium tracking-wide uppercase' : (isRace ? 'text-slate-400 tracking-wide uppercase' : 'text-purple-400/80')}>
                      {isPrimary ? 'Primary Event' : (isRace ? 'Secondary Race/Event' : workout.sport)}
                    </span>
                    {(isRace && workout.sport) && (
                      <span className="text-gray-500">{workout.sport}</span>
                    )}
                  </div>
                  {isPrimary && <AnalyzeUpcomingButton workout={workout} />}
                </div>
              );
            })}
          </div>
        </section>

        {/* ─── Today's Completed Workouts ─── */}
        <section className="space-y-6 section-reveal">
          <div className="flex items-center justify-between">
            <h2 className="text-2xl font-bold tracking-tight text-emerald-400 section-header">Completed Today</h2>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {todayWorkouts.done.length === 0 ? (
              <div className="col-span-full py-10 text-center text-gray-500 glass-panel border border-dashed border-gray-700/50">
                <div className="text-lg">No workouts completed today.</div>
              </div>
            ) : todayWorkouts.done.map((workout: CompletedWorkout, idx: number) => (
              <div key={idx} className="glass-panel hover-lift p-5 group relative border border-emerald-500/15 overflow-hidden stagger-item" style={{ '--stagger-index': idx } as React.CSSProperties}>
                <div className="absolute inset-0 bg-gradient-to-br from-emerald-500/8 to-transparent opacity-0 group-hover:opacity-100 transition-opacity duration-500 pointer-events-none" />
                <div className="flex justify-between items-start mb-2">
                  <h4 className="text-white font-medium">{workout.name}</h4>
                  <span className="text-xs px-2.5 py-1 bg-emerald-500/15 text-emerald-300 rounded-full border border-emerald-500/20">{extractType(workout.type || workout.activity_type) || "Activity"}</span>
                </div>
                <div className="mt-4 grid grid-cols-2 gap-2 text-sm">
                  {!!workout.duration && (
                    <div className="flex flex-col">
                      <span className="text-gray-500 text-xs">Duration</span>
                      <span className="text-gray-300">{(workout.duration / 60).toFixed(0)} min</span>
                    </div>
                  )}
                  {!!workout.distance && (
                    <div className="flex flex-col">
                      <span className="text-gray-500 text-xs">Distance</span>
                      <span className="text-gray-300">{(workout.distance / 1000).toFixed(1)} km</span>
                    </div>
                  )}
                  {!!workout.averageHR && (
                    <div className="flex flex-col">
                      <span className="text-gray-500 text-xs">Avg HR</span>
                      <span className="text-gray-300">{Math.round(workout.averageHR)} bpm</span>
                    </div>
                  )}
                </div>
                <AnalyzeButton workout={workout} />
              </div>
            ))}
          </div>
        </section>

        {/* ─── Weekly Strength Deltas ─── */}
        {weeklyDeltas.length > 0 && (
          <section className="space-y-6 section-reveal">
            <div className="flex items-center justify-between">
              <h2 className="text-2xl font-bold tracking-tight text-cyan-400 section-header">Week-over-Week Progression</h2>
            </div>
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
              {weeklyDeltas.map((delta, idx) => {
                const weightDiff = delta.this_week_weight - delta.last_week_weight;
                const repsDiff = delta.this_week_reps - delta.last_week_reps;
                const isUp = weightDiff > 0 || (weightDiff === 0 && repsDiff > 0);
                const isDown = weightDiff < 0 || (weightDiff === 0 && repsDiff < 0);

                return (
                  <div key={idx} className="glass-panel hover-lift p-5 group relative overflow-hidden stagger-item" style={{ '--stagger-index': idx } as React.CSSProperties}>
                    <div className="absolute inset-0 bg-gradient-to-br from-cyan-500/8 to-transparent opacity-0 group-hover:opacity-100 transition-opacity duration-500 pointer-events-none" />
                    <h4 className="text-gray-400 text-sm font-medium tracking-wider truncate" title={delta.exercise_name}>
                      {delta.exercise_name}
                    </h4>
                    <div className="mt-3 flex items-center justify-between">
                      <div className="flex items-baseline gap-2">
                        <span className="text-3xl font-extrabold tracking-tight text-white">{delta.this_week_weight.toFixed(1)}</span>
                        <span className="text-gray-500 text-sm">kg × {delta.this_week_reps}</span>
                      </div>
                      <span className={`text-lg font-bold ${isUp ? 'text-emerald-400' : isDown ? 'text-red-400' : 'text-gray-500'}`} style={{ filter: isUp ? 'drop-shadow(0 0 6px rgba(52,211,153,0.4))' : isDown ? 'drop-shadow(0 0 6px rgba(248,113,113,0.4))' : 'none' }}>
                        {isUp ? '↑' : isDown ? '↓' : '='}
                      </span>
                    </div>
                    {delta.last_week_weight > 0 && (
                      <div className="mt-2 pt-2 border-t border-white/5 text-xs text-gray-500">
                        Last week: {delta.last_week_weight.toFixed(1)}kg × {delta.last_week_reps}
                        {weightDiff !== 0 && (
                          <span className={`ml-2 ${weightDiff > 0 ? 'text-emerald-400' : 'text-red-400'}`}>
                            ({weightDiff > 0 ? '+' : ''}{weightDiff.toFixed(1)}kg)
                          </span>
                        )}
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          </section>
        )}

        {/* ─── Strength Progression (Personal Bests) ─── */}
        <section className="space-y-6 section-reveal">
          <div className="flex items-center justify-between">
            <h2 className="text-2xl font-bold tracking-tight section-header">Strength Progression (Personal Bests)</h2>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
            {data.length === 0 ? (
              <div className="col-span-full py-20 text-center text-gray-400 glass-panel">
                <div className="text-xl mb-2 font-medium">No historical progression data found.</div>
                <div className="text-sm">Ensure the Rust AI Coach API is running on <code className="text-red-500 bg-red-500/10 px-1.5 py-0.5 rounded">port 3001</code>.</div>
              </div>
            ) : data.map((item, idx) => (
              <div key={idx} className="glass-panel hover-lift p-5 group cursor-default relative overflow-hidden stagger-item" style={{ '--stagger-index': idx } as React.CSSProperties}>
                <div className="absolute inset-0 bg-gradient-to-br from-red-500/8 to-transparent opacity-0 group-hover:opacity-100 transition-opacity duration-500 pointer-events-none" />
                <h4 className="text-gray-400 text-sm font-medium tracking-wider truncate" title={item.exercise_name}>
                  {item.exercise_name}
                </h4>
                <div className="mt-4 flex items-center justify-between">
                  <div className="flex items-baseline gap-2">
                    <span className="text-4xl font-extrabold tracking-tight text-white group-hover:text-red-100 transition-colors duration-300">{item.max_weight.toFixed(1)}</span>
                    <span className="text-gray-500 text-sm font-medium">kg</span>
                  </div>
                  {item.history && item.history.length > 0 && (
                    <div className="w-20 h-10 right-2">
                      <Sparkline history={item.history} />
                    </div>
                  )}
                </div>
                <div className="mt-4 pt-4 border-t border-white/5 flex items-center justify-between text-sm">
                  <span className="text-gray-400">
                    <span className="text-emerald-400 font-bold">{item.reps}</span> reps
                  </span>
                  <span className="text-gray-600 truncate max-w-[120px]" title={item.date}>{item.date}</span>
                </div>
              </div>
            ))}
          </div>
        </section>

        {/* ─── AI Coach Chat ─── */}
        <section className="space-y-6 section-reveal">
          <Chat />
        </section>

        {/* ─── Muscle Map ─── */}
        <section className="space-y-6 section-reveal">
          <MuscleMap />
        </section>
      </div>
    </main>
  );
}
