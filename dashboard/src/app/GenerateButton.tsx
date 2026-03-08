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
        <div className="flex items-center gap-3">
            {result && (
                <span className={`text-[10px] uppercase tracking-wider font-semibold ${result.status === 'success' ? 'text-emerald-400' : 'text-red-400'}`}>
                    {result.status === 'success' ? 'Success' : 'Error'}
                </span>
            )}
            <button
                onClick={handleGenerate}
                disabled={loading}
                title="Generate a new AI-powered weekly workout plan based on your goals, recovery, and progression"
                className="flex items-center gap-2 px-3 py-1.5 glass-panel text-xs text-white uppercase tracking-wider hover:bg-white/10 transition-all border border-white/10 disabled:opacity-50 disabled:cursor-not-allowed hover:shadow-[0_0_15px_rgba(245,158,11,0.2)]"
            >
                {loading ? (
                    <>
                        <svg className="animate-spin h-3.5 w-3.5 text-white" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4"></circle>
                            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                        </svg>
                        <span>Generating...</span>
                    </>
                ) : (
                    <>
                        <span className="text-amber-400">✦</span>
                        <span>Generate Plan</span>
                    </>
                )}
            </button>
        </div>
    );
}
