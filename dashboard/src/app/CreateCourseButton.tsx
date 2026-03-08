'use client';

import React, { useState } from 'react';

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

export default function CreateCourseButton({ workout }: { workout: PlannedWorkout }) {
    const [loading, setLoading] = useState(false);
    const [result, setResult] = useState<{ courseName: string; distanceKm: number } | null>(null);
    const [error, setError] = useState<string | null>(null);

    const handleCreate = async (e: React.MouseEvent) => {
        e.stopPropagation();
        if (result) return;

        setLoading(true);
        setError(null);
        try {
            const res = await fetch('/api/course/create', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ workout })
            });
            const data = await res.json();
            if (!res.ok) {
                setError(data.message || 'Failed to create course');
            } else {
                setResult({ courseName: data.courseName, distanceKm: data.distanceKm });
            }
        } catch {
            setError('Network error or backend not running.');
        }
        setLoading(false);
    };

    return (
        <>
            <button
                onClick={handleCreate}
                disabled={loading || !!result}
                title={error || undefined}
                className={`inline-flex items-center gap-1.5 text-[10px] font-medium px-2.5 py-1 rounded-full transition-all whitespace-nowrap ${
                    result
                        ? 'bg-emerald-500/15 text-emerald-400 border border-emerald-500/25'
                        : error
                            ? 'bg-red-500/15 text-red-400 border border-red-500/25'
                            : 'bg-sky-500/10 text-sky-300 border border-sky-500/20 hover:bg-sky-500/20 hover:border-sky-500/35'
                } disabled:opacity-50`}
            >
                {loading ? (
                    <>
                        <span className="w-2.5 h-2.5 rounded-full border-[1.5px] border-sky-400 border-t-transparent animate-spin flex-shrink-0"></span>
                        Creating...
                    </>
                ) : result ? (
                    <>✓ {result.distanceKm.toFixed(1)} km</>
                ) : error ? (
                    <>✕ Failed</>
                ) : (
                    <>✦ Course</>
                )}
            </button>
        </>
    );
}
