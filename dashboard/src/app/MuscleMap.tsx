'use client';

import React, { useEffect, useMemo, useRef, useState } from 'react';
import Body, { ExtendedBodyPart } from '@mjcdev/react-body-highlighter';

type MuscleMapItem = {
    name: string;
    muscles: string[];
    frequency: number;
};

type TooltipInfo = {
    muscle: string;
    exercises: { name: string; frequency: number }[];
    totalSets: number;
};

const FATIGUE_COLORS = [
    "#fef08a", // yellow-300
    "#fde047", // yellow-400
    "#facc15", // yellow-500
    "#eab308", // yellow-600
    "#ca8a04", // yellow-700
    "#ea580c", // orange-600
    "#dc2626", // red-600
    "#b91c1c", // red-700
    "#991b1b", // red-800
    "#7f1d1d", // red-900
];

function formatExerciseName(name: string): string {
    return name
        .split('_')
        .map(word => word.charAt(0).toUpperCase() + word.slice(1).toLowerCase())
        .join(' ');
}

function formatMuscleName(slug: string): string {
    return slug
        .split('-')
        .map(word => word.charAt(0).toUpperCase() + word.slice(1))
        .join(' ');
}

export default function MuscleMap() {
    const [data, setData] = useState<MuscleMapItem[]>([]);
    const [loading, setLoading] = useState(true);
    const [gender, setGender] = useState<'male' | 'female'>('male');
    const [tooltipInfo, setTooltipInfo] = useState<TooltipInfo | null>(null);
    const [tooltipPos, setTooltipPos] = useState({ x: 0, y: 0 });
    const wrapperRef = useRef<HTMLDivElement>(null);
    const tooltipRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        async function fetchData() {
            try {
                const res = await fetch('/api/muscle_heatmap');
                if (res.ok) {
                    const json = await res.json();
                    setData(json);
                }
            } catch (err) {
                console.error("Failed to fetch muscle heatmap data", err);
            } finally {
                setLoading(false);
            }
        }
        fetchData();
    }, []);

    const slugMap: Record<string, string> = {
        'front-deltoids': 'deltoids',
        'back-deltoids': 'deltoids',
        'adductor': 'adductors',
        'abductors': 'adductors',
    };

    const muscleToExercises = useMemo(() => {
        const map: Record<string, { name: string; frequency: number }[]> = {};
        data.forEach(item => {
            item.muscles.forEach(muscle => {
                const mappedSlug = slugMap[muscle] || muscle;
                if (!map[mappedSlug]) {
                    map[mappedSlug] = [];
                }
                map[mappedSlug].push({
                    name: formatExerciseName(item.name),
                    frequency: item.frequency,
                });
            });
        });
        return map;
    }, [data]);

    const muscleFrequencies: Record<string, number> = {};
    data.forEach(item => {
        const freq = item.frequency || 1;
        item.muscles.forEach(muscle => {
            const mappedSlug = slugMap[muscle] || muscle;
            muscleFrequencies[mappedSlug] = (muscleFrequencies[mappedSlug] || 0) + freq;
        });
    });

    const bodyParts: ExtendedBodyPart[] = Object.entries(muscleFrequencies).map(([slug, freq]) => {
        const intensityStr = Math.min(Math.max(1, freq), 10);
        return {
            slug: slug as ExtendedBodyPart['slug'],
            intensity: intensityStr
        };
    });

    useEffect(() => {
        const wrapper = wrapperRef.current;
        if (!wrapper || Object.keys(muscleToExercises).length === 0) return;

        const handleMouseMove = (e: MouseEvent) => {
            const target = e.target as Element;
            let current: Element | null = target;
            let slug: string | null = null;
            while (current && current instanceof Element) {
                const id = current.getAttribute('id');
                if (id && muscleToExercises[id]) {
                    slug = id;
                    break;
                }
                current = current.parentElement;
            }

            if (slug) {
                const exercises = muscleToExercises[slug];
                const totalSets = exercises.reduce((sum, ex) => sum + ex.frequency, 0);
                const rect = wrapper.getBoundingClientRect();
                setTooltipPos({
                    x: e.clientX - rect.left + 16,
                    y: e.clientY - rect.top - 8,
                });
                setTooltipInfo({ muscle: slug, exercises, totalSets });
            } else {
                setTooltipInfo(null);
            }
        };

        const handleMouseLeave = () => {
            setTooltipInfo(null);
        };

        wrapper.addEventListener('mousemove', handleMouseMove);
        wrapper.addEventListener('mouseleave', handleMouseLeave);
        return () => {
            wrapper.removeEventListener('mousemove', handleMouseMove);
            wrapper.removeEventListener('mouseleave', handleMouseLeave);
        };
    }, [muscleToExercises]);

    if (loading) {
        return (
            <div className="glass-panel-elevated p-6 flex items-center justify-center min-h-[300px]">
                <div className="text-gray-400 animate-pulse">Loading Muscle Heatmap...</div>
            </div>
        );
    }

    return (
        <div className="glass-panel-elevated p-6 relative overflow-hidden">
            {/* Ambient glow behind */}
            <div className="ambient-glow bg-red-500" style={{ top: '50%', left: '50%', transform: 'translate(-50%, -50%)', width: '300px', height: '300px', opacity: 0.06 }} />

            <div className="flex flex-col md:flex-row justify-between items-start md:items-center mb-6 relative z-10">
                <div>
                    <h3 className="text-xl font-bold tracking-tight mb-2 section-header">Muscle Fatigue Heatmap</h3>
                    <p className="text-gray-400 text-sm">
                        Visualizing active working sets over the last 14 days. Yellow indicates low fatigue, deep red indicates high fatigue.
                    </p>
                </div>
                <div className="mt-4 md:mt-0 flex bg-white/[0.03] p-1 rounded-xl border border-white/8 backdrop-blur-sm">
                    <button
                        onClick={() => setGender('male')}
                        className={`px-4 py-1.5 text-sm font-medium rounded-lg transition-all ${gender === 'male' ? 'bg-red-500/15 text-red-400 border border-red-500/25 shadow-[0_0_10px_rgba(248,113,113,0.1)]' : 'text-gray-400 hover:text-gray-300 border border-transparent'}`}
                    >
                        Male
                    </button>
                    <button
                        onClick={() => setGender('female')}
                        className={`px-4 py-1.5 text-sm font-medium rounded-lg transition-all ${gender === 'female' ? 'bg-red-500/15 text-red-400 border border-red-500/25 shadow-[0_0_10px_rgba(248,113,113,0.1)]' : 'text-gray-400 hover:text-gray-300 border border-transparent'}`}
                    >
                        Female
                    </button>
                </div>
            </div>

            <div
                ref={wrapperRef}
                className="relative flex flex-col md:flex-row items-center justify-center gap-12 w-full z-10"
            >
                <div className="flex flex-col items-center w-full max-w-[250px]">
                    <h4 className="text-sm font-medium text-gray-400 mb-4 tracking-wider uppercase">Anterior (Front)</h4>
                    <Body
                        data={bodyParts}
                        side="front"
                        gender={gender}
                        colors={FATIGUE_COLORS}
                    />
                </div>

                <div className="flex flex-col items-center w-full max-w-[250px]">
                    <h4 className="text-sm font-medium text-gray-400 mb-4 tracking-wider uppercase">Posterior (Back)</h4>
                    <Body
                        data={bodyParts}
                        side="back"
                        gender={gender}
                        colors={FATIGUE_COLORS}
                    />
                </div>

                {/* Hover tooltip */}
                {tooltipInfo && (
                    <div
                        ref={tooltipRef}
                        className="pointer-events-none absolute z-50"
                        style={{
                            left: tooltipPos.x,
                            top: tooltipPos.y,
                            transform: 'translateY(-100%)',
                        }}
                    >
                        <div
                            className="rounded-2xl px-4 py-3 shadow-2xl border border-white/10 min-w-[180px]"
                            style={{
                                background: 'rgba(6, 6, 24, 0.94)',
                                backdropFilter: 'blur(20px)',
                                WebkitBackdropFilter: 'blur(20px)',
                                boxShadow: '0 8px 32px rgba(0, 0, 0, 0.4), 0 1px 0 rgba(255, 255, 255, 0.05) inset',
                            }}
                        >
                            <div className="text-sm font-semibold text-white mb-2 tracking-tight">
                                {formatMuscleName(tooltipInfo.muscle)}
                            </div>
                            <div className="space-y-1">
                                {tooltipInfo.exercises.map((ex, i) => (
                                    <div key={i} className="flex items-center justify-between gap-4 text-xs">
                                        <span className="text-gray-300">{ex.name}</span>
                                        <span className="text-gray-500 tabular-nums whitespace-nowrap">
                                            {ex.frequency} {ex.frequency === 1 ? 'set' : 'sets'}
                                        </span>
                                    </div>
                                ))}
                            </div>
                            <div className="mt-2 pt-2 border-t border-white/8 flex items-center justify-between text-xs">
                                <span className="text-gray-400">Total</span>
                                <span className="text-red-400 font-medium tabular-nums">
                                    {tooltipInfo.totalSets} {tooltipInfo.totalSets === 1 ? 'set' : 'sets'}
                                </span>
                            </div>
                        </div>
                    </div>
                )}
            </div>

            {
                data.length === 0 && (
                    <div className="text-center text-gray-500 mt-6 text-sm">
                        No active strength sets logged in the last 14 days.
                    </div>
                )
            }
        </div>
    );
}
