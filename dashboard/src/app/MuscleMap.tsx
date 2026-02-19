'use client';

import React, { useEffect, useState } from 'react';
import Model from 'react-body-highlighter';

import type { Muscle } from 'react-body-highlighter/dist/component/metadata';

type MuscleMapItem = {
    name: string;
    muscles: Muscle[];
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

    useEffect(() => {
        async function fetchData() {
            try {
                const res = await fetch('http://localhost:3001/api/muscle_heatmap');
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

    // The Model component expects data in the format of IExerciseData objects
    // where each object has { name: string, muscles: string[], frequency?: number }
    // Our backend happens to return exactly this format, but as custom types. We will coerce it.

    return (
        <div className="glass-panel p-6">
            <h3 className="text-xl font-bold tracking-tight mb-2">Muscle Fatigue Heatmap</h3>
            <p className="text-gray-400 text-sm mb-8">
                Visualizing active working sets over the last 14 days. Yellow indicates low fatigue, deep red indicates high fatigue.
            </p>

            {/* 
        The component svgStyle handles making it fit into the design.
        We provide a dark body color to match the dashboard's aesthetic.
      */}
            <div className="flex flex-col md:flex-row items-center justify-center gap-12 w-full">
                <div className="flex flex-col items-center">
                    <h4 className="text-sm font-medium text-gray-400 mb-4 tracking-wider uppercase">Anterior (Front)</h4>
                    <Model
                        data={data as any}
                        type="anterior"
                        bodyColor="#1f2937"
                        highlightedColors={FATIGUE_COLORS}
                        style={{ width: "100%", maxWidth: "250px" }}
                    />
                </div>

                <div className="flex flex-col items-center">
                    <h4 className="text-sm font-medium text-gray-400 mb-4 tracking-wider uppercase">Posterior (Back)</h4>
                    <Model
                        data={data as any}
                        type="posterior"
                        bodyColor="#1f2937"
                        highlightedColors={FATIGUE_COLORS}
                        style={{ width: "100%", maxWidth: "250px" }}
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
