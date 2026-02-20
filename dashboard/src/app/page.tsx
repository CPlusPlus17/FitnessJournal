import React from 'react';
import GenerateButton from './GenerateButton';
import MuscleMap from './MuscleMap';
import Chat from './Chat';

type ProgressionItem = {
  exercise_name: string;
  max_weight: number;
  reps: number;
  date: string;
  history?: { weight: number; reps: number; date: string }[];
};

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
  const strokeColorR = "rgba(167,139,250,0.6)"; // Purple dashed line with opacity for reps

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
      {/* Reps Line */}
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

      {/* Weight Line */}
      <polyline
        fill="none"
        stroke={strokeColorW}
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
        points={pointsW}
        style={{ filter: `drop-shadow(0px 2px 4px ${strokeColorW})` }}
        className="opacity-75 group-hover:opacity-100 transition-opacity"
      />
      <circle cx="0" cy={height - ((history[0].weight - minY_W) / (maxY_W - minY_W)) * height} r="2.5" fill={strokeColorW} className="opacity-75 group-hover:opacity-100 transition-opacity" />
      <circle cx={width} cy={height - ((history[history.length - 1].weight - minY_W) / (maxY_W - minY_W)) * height} r="2.5" fill={strokeColorW} className="opacity-75 group-hover:opacity-100 transition-opacity" />
    </svg>
  );
}

async function fetchProgression(): Promise<ProgressionItem[]> {
  try {
    const res = await fetch('http://localhost:3001/api/progression', { cache: 'no-store' });
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
    const res = await fetch('http://localhost:3001/api/recovery', { cache: 'no-store' });
    if (!res.ok) {
      return { body_battery: null, sleep_score: null, training_readiness: null, hrv_status: null, hrv_weekly_avg: null, hrv_last_night_avg: null, rhr_trend: [] };
    }
    return await res.json();
  } catch (err) {
    console.error("Fetch failed for recovery metrics.", err);
    return { body_battery: null, sleep_score: null, training_readiness: null, hrv_status: null, hrv_weekly_avg: null, hrv_last_night_avg: null, rhr_trend: [] };
  }
}

type TodayWorkoutsResponse = {
  done: any[];
  planned: any[];
};

async function fetchTodayWorkouts(): Promise<TodayWorkoutsResponse> {
  try {
    const res = await fetch('http://localhost:3001/api/workouts/today', { cache: 'no-store' });
    if (!res.ok) {
      return { done: [], planned: [] };
    }
    return await res.json();
  } catch (err) {
    console.error("Fetch failed for today workouts.", err);
    return { done: [], planned: [] };
  }
}

async function fetchUpcomingWorkouts(): Promise<any[]> {
  try {
    const res = await fetch('http://localhost:3001/api/workouts/upcoming', { cache: 'no-store' });
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
  const data = await fetchProgression();
  const recovery = await fetchRecovery();
  const todayWorkouts = await fetchTodayWorkouts();
  const upcomingWorkouts = await fetchUpcomingWorkouts();

  // Helper to check if a planned workout is already completed today
  const isWorkoutDone = (planned: any) => {
    const pTitle = (planned.title || planned.name || "").toLowerCase();
    const pSport = (planned.sport || planned.type || "").toLowerCase().replace(/_|\s/g, '');

    return todayWorkouts.done.some((done: any) => {
      const dName = (done.name || "").toLowerCase();
      const dType = (done.type || done.activity_type || done.sport || "").toLowerCase().replace(/_|\s/g, '');

      // Match by title/name (e.g., "Jegenstorf - Base" includes "Base")
      if (pTitle && dName && (dName.includes(pTitle) || pTitle.includes(dName))) {
        return true;
      }

      // Strict match by sport/type if title doesn't match and type exists
      if (pSport && dType && (dType.includes(pSport) || pSport.includes(dType))) {
        return true;
      }

      return false;
    });
  };

  const activePlannedWorkouts = todayWorkouts.planned.filter((w: any) => !isWorkoutDone(w));

  // Filter upcoming workouts: if they are planned for today, check if they are done
  const todayDates = new Set(todayWorkouts.planned.map((w: any) => w.date));
  const activeUpcomingWorkouts = upcomingWorkouts.filter((w: any) => {
    if (todayDates.has(w.date)) {
      return !isWorkoutDone(w);
    }
    return true;
  });

  return (
    <main className="min-h-screen p-8 md:p-24 selection:bg-red-500 selection:text-white pb-32">
      <div className="max-w-6xl mx-auto space-y-12">
        <header className="space-y-4">
          <h1 className="text-4xl md:text-6xl font-extrabold tracking-tight bg-clip-text text-transparent bg-gradient-to-r from-red-400 to-orange-500">
            Fitness Intelligence
          </h1>
          <p className="text-gray-400 text-lg md:text-xl max-w-2xl">
            Live AI Coaching Dashboard and Garmin Connect Integration.
          </p>
        </header>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-6 gap-6">
          <div className="glass-panel p-6 flex flex-col justify-between group">
            <h3 className="text-gray-400 font-medium tracking-wide">BODY BATTERY</h3>
            <div className="mt-4 flex items-end gap-2 text-white group-hover:text-red-400 transition-colors">
              <span className="text-5xl font-bold tracking-tighter">{recovery.body_battery ?? '--'}</span>
              <span className="text-gray-500 mb-1">/100</span>
            </div>
            <div className="mt-2 text-xs text-gray-500">Garmin Connect API</div>
          </div>
          <div className="glass-panel p-6 flex flex-col justify-between group">
            <h3 className="text-gray-400 font-medium tracking-wide">SLEEP SCORE</h3>
            <div className="mt-4 flex items-end gap-2 text-white group-hover:text-indigo-400 transition-colors">
              <span className="text-5xl font-bold tracking-tighter">{recovery.sleep_score ?? '--'}</span>
              <span className="text-gray-500 mb-1">/100</span>
            </div>
            <div className="mt-2 text-xs text-gray-500">Garmin Connect API</div>
          </div>
          <div className="glass-panel p-6 flex flex-col justify-between group">
            <h3 className="text-gray-400 font-medium tracking-wide">TRAINING READINESS</h3>
            <div className="mt-4 flex items-end gap-2 text-white group-hover:text-emerald-400 transition-colors">
              <span className="text-5xl font-bold tracking-tighter">{recovery.training_readiness ?? '--'}</span>
              <span className="text-gray-500 mb-1">/100</span>
            </div>
            <div className="mt-2 text-xs text-gray-500">Garmin Connect API</div>
          </div>
          <div className="glass-panel p-6 flex flex-col justify-between group">
            <h3 className="text-gray-400 font-medium tracking-wide">HRV STATUS</h3>
            <div className="mt-4 flex flex-col gap-1 text-white group-hover:text-purple-400 transition-colors">
              <div className="text-4xl font-bold tracking-tight py-2 text-purple-400">
                {recovery.hrv_last_night_avg ?? '--'} <span className="text-base font-normal text-gray-500">ms</span>
              </div>
              <div className="text-sm text-gray-400 flex flex-col gap-1 mt-1">
                <div className="flex justify-between">
                  <span>Status</span>
                  <span className="text-white font-medium uppercase">{recovery.hrv_status ?? '--'}</span>
                </div>
                <div className="flex justify-between">
                  <span>7-Day Avg</span>
                  <span className="text-white font-medium">{recovery.hrv_weekly_avg ?? '--'} ms</span>
                </div>
              </div>
            </div>
            <div className="mt-4 text-xs text-gray-500 pt-2 border-t border-white/5">Garmin Connect API</div>
          </div>
          <div className="glass-panel p-6 flex flex-col justify-between group">
            <h3 className="text-gray-400 font-medium tracking-wide">RESTING HR</h3>
            <div className="mt-4 flex flex-col gap-1 text-white group-hover:text-rose-400 transition-colors">
              <div className="text-4xl font-bold tracking-tight py-2 text-rose-400">
                {recovery.rhr_trend && recovery.rhr_trend.length > 0 ? recovery.rhr_trend[recovery.rhr_trend.length - 1] : '--'} <span className="text-base font-normal text-gray-500">bpm</span>
              </div>
              <div className="h-10 w-full mt-2">
                {recovery.rhr_trend && recovery.rhr_trend.length > 0 && (
                  <Sparkline history={recovery.rhr_trend.map((val, idx) => ({ weight: val, date: String(idx) }))} />
                )}
              </div>
            </div>
            <div className="mt-4 text-xs text-gray-500 pt-2 border-t border-white/5">Garmin Connect API</div>
          </div>
          <div className="h-full">
            <GenerateButton />
          </div>
        </div>

        <section className="space-y-6 mt-6">
          <Chat />
        </section>

        <section className="space-y-6">
          <MuscleMap />
        </section>

        {/* Today's Planned Workouts */}
        <section className="space-y-6">
          <div className="flex items-center justify-between">
            <h2 className="text-2xl font-bold tracking-tight text-indigo-400">Planned Today</h2>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {activePlannedWorkouts.length === 0 ? (
              <div className="col-span-full py-10 text-center text-gray-500 glass-panel border border-dashed border-gray-700">
                <div className="text-lg">
                  {todayWorkouts.planned.length > 0 ? "All planned workouts completed for today! ✅" : "No workouts planned for today."}
                </div>
              </div>
            ) : activePlannedWorkouts.map((workout: any, idx: number) => (
              <div key={idx} className="glass-panel p-5 group relative border border-indigo-500/20">
                <div className="absolute inset-0 bg-gradient-to-br from-indigo-500/10 to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />
                <div className="flex justify-between items-start mb-2">
                  <h4 className="text-white font-medium">{workout.title || workout.description || "Training"}</h4>
                  <span className="text-xs px-2 py-1 bg-indigo-500/20 text-indigo-300 rounded-full">{workout.type || workout.sport || "Workout"}</span>
                </div>
                {(workout.duration || workout.distance) ? (
                  <div className="mt-4 flex items-center gap-4 text-sm text-gray-400">
                    {workout.duration ? <span>{workout.duration.toFixed(0)} min</span> : null}
                    {workout.distance ? <span>{workout.distance.toFixed(1)} km</span> : null}
                  </div>
                ) : null}
              </div>
            ))}
          </div>
        </section>

        {/* Upcoming Workouts */}
        <section className="space-y-6">
          <div className="flex items-center justify-between">
            <h2 className="text-2xl font-bold tracking-tight text-purple-400">Upcoming Schedule</h2>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {activeUpcomingWorkouts.length === 0 ? (
              <div className="col-span-full py-10 text-center text-gray-500 glass-panel border border-dashed border-gray-700">
                <div className="text-lg">No upcoming workouts planned.</div>
              </div>
            ) : activeUpcomingWorkouts.map((workout: any, idx: number) => {
              const isRace = workout.is_race === true || workout.type === 'race' || workout.type === 'event' || workout.type === 'primaryEvent';
              const isPrimary = workout.primary_event === true || workout.type === 'primaryEvent';

              let borderClass = "border-purple-500/20";
              let bgHoverClass = "from-purple-500/10";
              let badgeBg = "bg-purple-500/20 text-purple-300";

              if (isPrimary) {
                borderClass = "border-amber-500/50 shadow-[0_0_15px_rgba(245,158,11,0.2)]";
                bgHoverClass = "from-amber-500/20";
                badgeBg = "bg-amber-500/20 text-amber-300 font-bold border border-amber-500/50";
              } else if (isRace) {
                borderClass = "border-slate-300/40 shadow-[0_0_10px_rgba(203,213,225,0.1)]";
                bgHoverClass = "from-slate-400/20";
                badgeBg = "bg-slate-400/20 text-slate-200 font-medium border border-slate-400/30";
              }

              return (
                <div key={idx} className={`glass-panel p-5 group relative border transition-all ${borderClass}`}>
                  <div className={`absolute inset-0 bg-gradient-to-br ${bgHoverClass} to-transparent opacity-0 group-hover:opacity-100 transition-opacity`} />

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
                    <span className={`text-xs px-2 py-1 rounded-full whitespace-nowrap ml-2 ${badgeBg}`}>
                      {new Date(workout.date).toLocaleDateString(undefined, { weekday: 'short', month: 'short', day: 'numeric' })}
                    </span>
                  </div>
                  {(workout.duration || workout.distance) ? (
                    <div className="mt-4 flex items-center gap-4 text-sm text-gray-400">
                      {workout.duration ? <span>{workout.duration.toFixed(0)} min</span> : null}
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
                </div>
              );
            })}
          </div>
        </section>

        {/* Today's Completed Workouts */}
        <section className="space-y-6">
          <div className="flex items-center justify-between">
            <h2 className="text-2xl font-bold tracking-tight text-emerald-400">Completed Today</h2>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {todayWorkouts.done.length === 0 ? (
              <div className="col-span-full py-10 text-center text-gray-500 glass-panel border border-dashed border-gray-700">
                <div className="text-lg">No workouts completed today.</div>
              </div>
            ) : todayWorkouts.done.map((workout: any, idx: number) => (
              <div key={idx} className="glass-panel p-5 group relative border border-emerald-500/20">
                <div className="absolute inset-0 bg-gradient-to-br from-emerald-500/10 to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />
                <div className="flex justify-between items-start mb-2">
                  <h4 className="text-white font-medium">{workout.name}</h4>
                  <span className="text-xs px-2 py-1 bg-emerald-500/20 text-emerald-300 rounded-full">{workout.type || "Activity"}</span>
                </div>
                <div className="mt-4 grid grid-cols-2 gap-2 text-sm">
                  {workout.duration && (
                    <div className="flex flex-col">
                      <span className="text-gray-500 text-xs">Duration</span>
                      <span className="text-gray-300">{(workout.duration / 60).toFixed(0)} min</span>
                    </div>
                  )}
                  {workout.distance && (
                    <div className="flex flex-col">
                      <span className="text-gray-500 text-xs">Distance</span>
                      <span className="text-gray-300">{(workout.distance / 1000).toFixed(1)} km</span>
                    </div>
                  )}
                  {workout.averageHR && (
                    <div className="flex flex-col">
                      <span className="text-gray-500 text-xs">Avg HR</span>
                      <span className="text-gray-300">{Math.round(workout.averageHR)} bpm</span>
                    </div>
                  )}
                </div>
              </div>
            ))}
          </div>
        </section>

        <section className="space-y-6">
          <div className="flex items-center justify-between">
            <h2 className="text-2xl font-bold tracking-tight">Strength Progression (Personal Bests)</h2>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
            {data.length === 0 ? (
              <div className="col-span-full py-20 text-center text-gray-400 glass-panel">
                <div className="text-xl mb-2 font-medium">No historical progression data found.</div>
                <div className="text-sm">Ensure the Rust AI Coach API is running on <code className="text-red-500 bg-red-500/10 px-1 py-0.5 rounded">port 3001</code>.</div>
              </div>
            ) : data.map((item, idx) => (
              <div key={idx} className="glass-panel p-5 group cursor-default relative overflow-hidden">
                <div className="absolute inset-0 bg-gradient-to-br from-red-500/10 to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />
                <h4 className="text-gray-400 text-sm font-medium tracking-wider truncate" title={item.exercise_name}>
                  {item.exercise_name}
                </h4>
                <div className="mt-4 flex items-center justify-between">
                  <div className="flex items-baseline gap-2">
                    <span className="text-4xl font-extrabold tracking-tight text-white group-hover:text-red-100 transition-colors">{item.max_weight.toFixed(1)}</span>
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
      </div>
    </main>
  );
}
