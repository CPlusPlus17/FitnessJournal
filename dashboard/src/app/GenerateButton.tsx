'use client';

import React, { useState } from 'react';

export default function GenerateButton() {
    const [loading, setLoading] = useState(false);
    const [result, setResult] = useState<{ status: string, message: string } | null>(null);

    const handleGenerate = async () => {
        setLoading(true);
        setResult(null);
        try {
            const res = await fetch('http://localhost:3001/api/generate', {
                method: 'POST',
            });
            const data = await res.json();
            setResult(data);
        } catch (err) {
            setResult({ status: 'error', message: 'Network error or Rust backend not running.' });
        }
        setLoading(false);
    };

    return (
        <div className="glass-panel p-6 flex flex-col justify-between group relative overflow-hidden h-full">
            <div className={`absolute -right-4 -top-12 w-32 h-32 rounded-full blur-2xl transition-all ${loading ? 'bg-amber-500/40 animate-pulse' : 'bg-red-500/20 group-hover:bg-red-500/30'
                }`}></div>
            <h3 className="text-gray-400 font-medium tracking-wide z-10">LLM COACH WORKOUTS</h3>

            <div className="mt-4 flex flex-col gap-4 z-10 w-full">
                <button
                    onClick={handleGenerate}
                    disabled={loading}
                    className="w-full bg-white/10 hover:bg-white/20 text-white font-medium py-2 rounded-full transition-all border border-white/10 hover:border-white/30 disabled:opacity-50 disabled:cursor-not-allowed"
                >
                    {loading ? 'Generating...' : 'Regenerate Plan'}
                </button>
                {result && (
                    <div className={`text-sm ${result.status === 'success' ? 'text-emerald-400' : 'text-red-400'}`}>
                        {result.message}
                    </div>
                )}
            </div>
            <div className="mt-4 text-xs text-gray-500 z-10">Gemini 3.1 Pro Preview</div>
        </div>
    );
}
