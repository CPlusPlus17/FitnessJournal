'use client';

import React, { useEffect, useState } from 'react';
import Body, { ExtendedBodyPart } from '@mjcdev/react-body-highlighter';

type MuscleMapItem = {
    name: string;
    muscles: string[];
    frequency: number;
};

// Adjust color scale to be from yellow/orange to deep red based on frequency of active sets
// frequency 1 = index 0 (yellow). frequency 10+ = index 9 (deep red)
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

export default function MuscleMap() {
    const [data, setData] = useState<MuscleMapItem[]>([]);
    const [loading, setLoading] = useState(true);
    const [gender, setGender] = useState<'male' | 'female'>('male');

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

    if (loading) {
        return (
            <div className="glass-panel p-6 flex items-center justify-center min-h-[300px]">
                <div className="text-gray-400 animate-pulse">Loading Muscle Heatmap...</div>
            </div>
        );
    }

    // Map old muscle names to the new package's accepted slugs
    const slugMap: Record<string, string> = {
        'front-deltoids': 'deltoids',
        'back-deltoids': 'deltoids',
        'adductor': 'adductors',
        'abductors': 'adductors',
    };

    // Aggregate frequencies per muscle
    const muscleFrequencies: Record<string, number> = {};
    data.forEach(item => {
        const freq = item.frequency || 1;
        item.muscles.forEach(muscle => {
            const mappedSlug = slugMap[muscle] || muscle;
            muscleFrequencies[mappedSlug] = (muscleFrequencies[mappedSlug] || 0) + freq;
        });
    });

    const bodyParts: ExtendedBodyPart[] = Object.entries(muscleFrequencies).map(([slug, freq]) => {
        // Find correct mapping to intensity (1 to 10 for FATIGUE_COLORS array length 10)
        const intensityStr = Math.min(Math.max(1, freq), 10);
        return {
            slug: slug as ExtendedBodyPart['slug'],
            intensity: intensityStr
        };
    });

    return (
        <div className="glass-panel p-6">
            <div className="flex flex-col md:flex-row justify-between items-start md:items-center mb-6">
                <div>
                    <h3 className="text-xl font-bold tracking-tight mb-2">Muscle Fatigue Heatmap</h3>
                    <p className="text-gray-400 text-sm">
                        Visualizing active working sets over the last 14 days. Yellow indicates low fatigue, deep red indicates high fatigue.
                    </p>
                </div>
                <div className="mt-4 md:mt-0 flex bg-gray-800/50 p-1 rounded-lg border border-white/10">
                    <button
                        onClick={() => setGender('male')}
                        className={`px-4 py-1.5 text-sm font-medium rounded-md transition-all ${gender === 'male' ? 'bg-red-500/20 text-red-400 border border-red-500/30' : 'text-gray-400 hover:text-gray-300'}`}
                    >
                        Male
                    </button>
                    <button
                        onClick={() => setGender('female')}
                        className={`px-4 py-1.5 text-sm font-medium rounded-md transition-all ${gender === 'female' ? 'bg-red-500/20 text-red-400 border border-red-500/30' : 'text-gray-400 hover:text-gray-300'}`}
                    >
                        Female
                    </button>
                </div>
            </div>

            {/* 
        The component svgStyle handles making it fit into the design.
        We provide a dark body color to match the dashboard's aesthetic.
      */}
            <div className="flex flex-col md:flex-row items-center justify-center gap-12 w-full">
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
            </div>

            {
                data.length === 0 && (
                    <div className="text-center text-gray-500 mt-6 text-sm">
                        No active strength sets logged in the last 14 days.
                    </div>
                )
            }
        </div >
    );
}
