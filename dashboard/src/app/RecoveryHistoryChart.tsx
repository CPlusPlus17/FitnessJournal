"use client";

import React, { useMemo } from 'react';
import {
    AreaChart,
    Area,
    XAxis,
    YAxis,
    CartesianGrid,
    Tooltip,
    ResponsiveContainer,
    Legend
} from 'recharts';

export type RecoveryHistoryEntry = {
    date: string;
    body_battery: number | null;
    sleep_score: number | null;
    training_readiness: number | null;
    hrv_last_night_avg: number | null;
    hrv_status: string | null;
    rhr: number | null;
};

export default function RecoveryHistoryChart({ data }: { data: RecoveryHistoryEntry[] }) {
    const chartData = useMemo(() => {
        if (!data || data.length === 0) return [];
        return data.map((d) => ({
            ...d,
            shortDate: new Date(d.date).toLocaleDateString(undefined, { weekday: 'short', month: 'short', day: 'numeric' }),
        }));
    }, [data]);

    if (chartData.length === 0) {
        return (
            <div className="flex items-center justify-center h-64 text-gray-500 border border-dashed border-gray-700/50 rounded-2xl">
                No recovery history available
            </div>
        );
    }

    return (
        <div className="w-full h-[300px]">
            <ResponsiveContainer width="100%" height="100%">
                <AreaChart data={chartData} margin={{ top: 10, right: 10, left: -20, bottom: 0 }}>
                    <defs>
                        <linearGradient id="colorBb" x1="0" y1="0" x2="0" y2="1">
                            <stop offset="5%" stopColor="#f87171" stopOpacity={0.35} />
                            <stop offset="95%" stopColor="#f87171" stopOpacity={0} />
                        </linearGradient>
                        <linearGradient id="colorSleep" x1="0" y1="0" x2="0" y2="1">
                            <stop offset="5%" stopColor="#818cf8" stopOpacity={0.35} />
                            <stop offset="95%" stopColor="#818cf8" stopOpacity={0} />
                        </linearGradient>
                        <linearGradient id="colorTr" x1="0" y1="0" x2="0" y2="1">
                            <stop offset="5%" stopColor="#34d399" stopOpacity={0.35} />
                            <stop offset="95%" stopColor="#34d399" stopOpacity={0} />
                        </linearGradient>
                        {/* Glow filters for chart lines */}
                        <filter id="glowRed" x="-20%" y="-20%" width="140%" height="140%">
                            <feGaussianBlur stdDeviation="3" result="blur" />
                            <feMerge>
                                <feMergeNode in="blur" />
                                <feMergeNode in="SourceGraphic" />
                            </feMerge>
                        </filter>
                        <filter id="glowBlue" x="-20%" y="-20%" width="140%" height="140%">
                            <feGaussianBlur stdDeviation="3" result="blur" />
                            <feMerge>
                                <feMergeNode in="blur" />
                                <feMergeNode in="SourceGraphic" />
                            </feMerge>
                        </filter>
                        <filter id="glowGreen" x="-20%" y="-20%" width="140%" height="140%">
                            <feGaussianBlur stdDeviation="3" result="blur" />
                            <feMerge>
                                <feMergeNode in="blur" />
                                <feMergeNode in="SourceGraphic" />
                            </feMerge>
                        </filter>
                    </defs>
                    <XAxis dataKey="shortDate" stroke="#6b7280" fontSize={11} tickLine={false} axisLine={false} />
                    <YAxis stroke="#6b7280" fontSize={11} tickLine={false} axisLine={false} domain={[0, 100]} />
                    <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="rgba(255,255,255,0.04)" />
                    <Tooltip
                        contentStyle={{
                            backgroundColor: 'rgba(6, 6, 24, 0.94)',
                            backdropFilter: 'blur(20px)',
                            borderColor: 'rgba(255,255,255,0.1)',
                            borderRadius: '16px',
                            color: '#fff',
                            boxShadow: '0 8px 32px rgba(0, 0, 0, 0.4)',
                        }}
                        itemStyle={{ color: '#fff' }}
                    />
                    <Legend iconType="circle" wrapperStyle={{ paddingTop: '20px' }} />
                    <Area type="monotone" name="Body Battery" dataKey="body_battery" stroke="#f87171" strokeWidth={2} fillOpacity={1} fill="url(#colorBb)" filter="url(#glowRed)" />
                    <Area type="monotone" name="Sleep Score" dataKey="sleep_score" stroke="#818cf8" strokeWidth={2} fillOpacity={1} fill="url(#colorSleep)" filter="url(#glowBlue)" />
                    <Area type="monotone" name="Readiness" dataKey="training_readiness" stroke="#34d399" strokeWidth={2} fillOpacity={1} fill="url(#colorTr)" filter="url(#glowGreen)" />
                </AreaChart>
            </ResponsiveContainer>
        </div>
    );
}
