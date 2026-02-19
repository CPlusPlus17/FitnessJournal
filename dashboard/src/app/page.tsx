import React from 'react';
import GenerateButton from './GenerateButton';
import MuscleMap from './MuscleMap';

type ProgressionItem = {
  exercise_name: string;
  max_weight: number;
  reps: number;
  date: string;
};

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
};

async function fetchRecovery(): Promise<RecoveryItem> {
  try {
    const res = await fetch('http://localhost:3001/api/recovery', { cache: 'no-store' });
    if (!res.ok) {
      return { body_battery: null, sleep_score: null };
    }
    return await res.json();
  } catch (err) {
    console.error("Fetch failed for recovery metrics.", err);
    return { body_battery: null, sleep_score: null };
  }
}

export default async function Dashboard() {
  const data = await fetchProgression();
  const recovery = await fetchRecovery();

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

        <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
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
          <div className="h-full">
            <GenerateButton />
          </div>
        </div>

        <section className="space-y-6">
          <MuscleMap />
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
                <div className="mt-4 flex items-baseline gap-2">
                  <span className="text-4xl font-extrabold tracking-tight text-white group-hover:text-red-100 transition-colors">{item.max_weight.toFixed(1)}</span>
                  <span className="text-gray-500 text-sm font-medium">kg</span>
                </div>
                <div className="mt-4 pt-4 border-t border-white/5 flex items-center justify-between text-sm">
                  <span className="text-gray-400">
                    <span className="text-emerald-400 font-bold">{item.reps}</span> reps
                  </span>
                  <span className="text-gray-600">{item.date}</span>
                </div>
              </div>
            ))}
          </div>
        </section>
      </div>
    </main>
  );
}
