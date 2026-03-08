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

    const runAnalysis = async () => {
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
                setError(data.message || 'Failed to analyze');
            } else {
                setAnalysis(data.analysis);
            }
        } catch {
            setError('Network error or backend not running.');
        }
        setLoading(false);
    };

    const handleAnalyze = async (e: React.MouseEvent) => {
        e.stopPropagation();
        if (analysis) {
            setAnalysis(null);
            return;
        }
        await runAnalysis();
    };

    const handleReanalyze = async (e: React.MouseEvent) => {
        e.stopPropagation();
        await runAnalysis();
    };

    return (
        <>
            <button
                onClick={handleAnalyze}
                disabled={loading}
                title={error || undefined}
                className="inline-flex items-center gap-1.5 text-[10px] font-medium px-2.5 py-1 rounded-full transition-all whitespace-nowrap bg-amber-500/10 text-amber-300 border border-amber-500/20 hover:bg-amber-500/20 hover:border-amber-500/35 disabled:opacity-50"
            >
                {loading ? (
                    <>
                        <span className="w-2.5 h-2.5 rounded-full border-[1.5px] border-amber-400 border-t-transparent animate-spin flex-shrink-0"></span>
                        Analyzing...
                    </>
                ) : analysis ? (
                    'Hide'
                ) : (
                    '✦ Analyze'
                )}
            </button>

            {error && (
                <div className="mt-2 text-xs text-red-400 bg-red-400/8 p-2 rounded-lg border border-red-500/15 col-span-full">
                    {error}
                </div>
            )}

            {analysis && (
                <div className="mt-2 text-sm text-gray-300 bg-black/20 p-4 rounded-xl border border-amber-500/15 prose prose-invert prose-amber prose-sm max-w-none overflow-auto max-h-96 custom-scrollbar text-left backdrop-blur-sm col-span-full" style={{ animation: 'fadeSlideUp 0.4s cubic-bezier(0.22, 1, 0.36, 1)' }}>
                    <ReactMarkdown>{analysis}</ReactMarkdown>
                    <div className="mt-3 pt-2 border-t border-amber-500/10 not-prose flex justify-end">
                        <button
                            onClick={handleReanalyze}
                            disabled={loading}
                            className="inline-flex items-center gap-1.5 text-[10px] font-medium px-2.5 py-1 rounded-full transition-all whitespace-nowrap bg-amber-500/10 text-amber-300 border border-amber-500/20 hover:bg-amber-500/20 hover:border-amber-500/35 disabled:opacity-50"
                        >
                            {loading ? (
                                <>
                                    <span className="w-2.5 h-2.5 rounded-full border-[1.5px] border-amber-400 border-t-transparent animate-spin flex-shrink-0"></span>
                                    Analyzing...
                                </>
                            ) : (
                                '↻ Re-analyze'
                            )}
                        </button>
                    </div>
                </div>
            )}
        </>
    );
}
