'use client';

import React, { useState } from 'react';
import ReactMarkdown from 'react-markdown';

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

export default function AnalyzeUpcomingButton({ workout }: { workout: PlannedWorkout }) {
    const [loading, setLoading] = useState(false);
    const [analysis, setAnalysis] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);

    const handleAnalyze = async (e: React.MouseEvent) => {
        e.stopPropagation();

        if (analysis) {
            setAnalysis(null);
            return;
        }

        setLoading(true);
        setError(null);
        try {
            const res = await fetch('/api/analyze/upcoming', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ workout })
            });
            const data = await res.json();
            if (!res.ok) {
                setError(data.message || 'Failed to analyze primary event');
            } else {
                setAnalysis(data.analysis);
            }
        } catch {
            setError('Network error or backend not running.');
        }
        setLoading(false);
    };

    return (
        <div className="mt-4 pt-4 border-t border-white/8 relative z-10 w-full">
            <button
                onClick={handleAnalyze}
                disabled={loading}
                className="w-full text-xs bg-amber-500/10 hover:bg-amber-500/20 text-amber-300 font-medium py-2.5 rounded-xl transition-all border border-amber-500/15 hover:border-amber-500/30 disabled:opacity-40 flex justify-center items-center gap-2 hover:shadow-[0_0_16px_rgba(245,158,11,0.1)]"
            >
                {loading ? (
                    <>
                        <span className="w-3 h-3 rounded-full border-2 border-amber-400 border-t-transparent animate-spin flex-shrink-0"></span>
                        Analyzing Event...
                    </>
                ) : analysis ? (
                    'Hide Analysis'
                ) : (
                    '⭐ Analyze Event Preparation'
                )}
            </button>

            {error && (
                <div className="mt-3 text-xs text-red-400 bg-red-400/8 p-3 rounded-xl border border-red-500/15">
                    {error}
                </div>
            )}

            {analysis && (
                <div className="mt-4 text-sm text-gray-300 bg-black/20 p-4 rounded-xl border border-amber-500/15 prose prose-invert prose-amber prose-sm max-w-none overflow-auto max-h-96 custom-scrollbar text-left backdrop-blur-sm" style={{ animation: 'fadeSlideUp 0.4s cubic-bezier(0.22, 1, 0.36, 1)' }}>
                    <ReactMarkdown>{analysis}</ReactMarkdown>
                </div>
            )}
        </div>
    );
}
