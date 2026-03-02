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

        // Toggle off if already showing
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
        <div className="mt-4 pt-4 border-t border-white/10 relative z-10 w-full">
            <button
                onClick={handleAnalyze}
                disabled={loading}
                className="w-full text-xs bg-amber-500/10 hover:bg-amber-500/20 text-amber-300 font-medium py-2 rounded transition-all border border-amber-500/20 hover:border-amber-500/40 disabled:opacity-50 flex justify-center items-center gap-2"
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
                <div className="mt-3 text-xs text-red-400 bg-red-400/10 p-3 rounded">
                    {error}
                </div>
            )}

            {analysis && (
                <div className="mt-4 text-sm text-gray-300 bg-black/20 p-4 rounded-md border border-amber-500/20 prose prose-invert prose-amber prose-sm max-w-none overflow-auto max-h-96 custom-scrollbar text-left">
                    <ReactMarkdown>{analysis}</ReactMarkdown>
                </div>
            )}
        </div>
    );
}
