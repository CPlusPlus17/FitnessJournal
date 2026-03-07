'use client';

import React, { useState } from 'react';

export default function GenerateButton() {
    const [loading, setLoading] = useState(false);
    const [result, setResult] = useState<{ status: string, message: string } | null>(null);

    const handleGenerate = async () => {
        setLoading(true);
        setResult(null);
        try {
            const res = await fetch('/api/generate', {
                method: 'POST',
            });
            const data = await res.json();
            setResult(data);
        } catch {
            setResult({ status: 'error', message: 'Network error or Rust backend not running.' });
        }
        setLoading(false);
    };

    return (
        <div className="glass-panel hover-lift p-6 flex flex-col justify-between group relative overflow-hidden h-full">
            <div className={`ambient-glow ${loading ? 'bg-amber-500' : 'bg-red-500'} -right-6 -top-10`}
                style={{
                    width: '140px', height: '140px',
                    animation: loading ? 'pulseGlow 1.5s ease-in-out infinite' : undefined,
                    opacity: loading ? 0.3 : 0.1,
                    transition: 'opacity 0.5s ease',
                }}
            />
            <h3 className="text-gray-400 font-medium tracking-wide z-10 text-xs uppercase">LLM Coach Workouts</h3>

            <div className="mt-4 flex flex-col gap-4 z-10 w-full">
                <button
                    onClick={handleGenerate}
                    disabled={loading}
                    className="w-full bg-white/8 hover:bg-white/15 text-white font-medium py-2.5 rounded-xl transition-all border border-white/8 hover:border-white/20 disabled:opacity-40 disabled:cursor-not-allowed hover:shadow-[0_0_20px_rgba(248,113,113,0.1)] glow-button"
                >
                    {loading ? 'Generating...' : 'Regenerate Plan'}
                </button>
                {result && (
                    <div className={`text-sm ${result.status === 'success' ? 'text-emerald-400' : 'text-red-400'}`}>
                        {result.message}
                    </div>
                )}
            </div>
            <div className="mt-4 text-[10px] text-gray-600 z-10 uppercase tracking-widest">Gemini 3.1 Pro Preview</div>
        </div>
    );
}
