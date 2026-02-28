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
            <div className="flex items-center justify-center h-64 text-gray-500 border border-dashed border-gray-700 rounded-lg">
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
                            <stop offset="5%" stopColor="#f87171" stopOpacity={0.4} />
                            <stop offset="95%" stopColor="#f87171" stopOpacity={0} />
                        </linearGradient>
                        <linearGradient id="colorSleep" x1="0" y1="0" x2="0" y2="1">
                            <stop offset="5%" stopColor="#818cf8" stopOpacity={0.4} />
                            <stop offset="95%" stopColor="#818cf8" stopOpacity={0} />
                        </linearGradient>
                        <linearGradient id="colorTr" x1="0" y1="0" x2="0" y2="1">
                            <stop offset="5%" stopColor="#34d399" stopOpacity={0.4} />
                            <stop offset="95%" stopColor="#34d399" stopOpacity={0} />
                        </linearGradient>
                    </defs>
                    <XAxis dataKey="shortDate" stroke="#9ca3af" fontSize={12} tickLine={false} axisLine={false} />
                    <YAxis stroke="#9ca3af" fontSize={12} tickLine={false} axisLine={false} domain={[0, 100]} />
                    <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="#374151" />
                    <Tooltip
                        contentStyle={{ backgroundColor: 'rgba(17, 24, 39, 0.9)', borderColor: '#374151', borderRadius: '8px', color: '#fff' }}
                        itemStyle={{ color: '#fff' }}
                    />
                    <Legend iconType="circle" wrapperStyle={{ paddingTop: '20px' }} />
                    <Area type="monotone" name="Body Battery" dataKey="body_battery" stroke="#f87171" strokeWidth={2} fillOpacity={1} fill="url(#colorBb)" />
                    <Area type="monotone" name="Sleep Score" dataKey="sleep_score" stroke="#818cf8" strokeWidth={2} fillOpacity={1} fill="url(#colorSleep)" />
                    <Area type="monotone" name="Readiness" dataKey="training_readiness" stroke="#34d399" strokeWidth={2} fillOpacity={1} fill="url(#colorTr)" />
                </AreaChart>
            </ResponsiveContainer>
        </div>
    );
}
