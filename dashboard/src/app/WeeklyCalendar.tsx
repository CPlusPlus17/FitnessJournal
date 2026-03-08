'use client';

import React, { useState } from 'react';
import CreateCourseButton from './CreateCourseButton';

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
    workout_detail?: any;
};

type WeekActivity = {
    name?: string;
    type?: unknown;
    activity_type?: unknown;
    sport?: unknown;
    startTimeLocal?: string;
    duration?: number;
    distance?: number;
    averageHR?: number;
    maxHR?: number;
    calories?: number;
    averageSpeed?: number;
    elevationGain?: number;
    avgPower?: number;
    sets?: any;
};

type WeeklyCalendarProps = {
    upcomingWorkouts: PlannedWorkout[];
    todayPlanned: PlannedWorkout[];
    completedCount: number;
    weekActivities?: WeekActivity[];
};

const SPORT_COLORS: Record<string, { bg: string; text: string; border: string }> = {
    strength: { bg: 'bg-red-500/15', text: 'text-red-300', border: 'border-red-500/25' },
    running: { bg: 'bg-green-500/15', text: 'text-green-300', border: 'border-green-500/25' },
    cycling: { bg: 'bg-amber-500/15', text: 'text-amber-300', border: 'border-amber-500/25' },
    swimming: { bg: 'bg-cyan-500/15', text: 'text-cyan-300', border: 'border-cyan-500/25' },
    yoga: { bg: 'bg-purple-500/15', text: 'text-purple-300', border: 'border-purple-500/25' },
    rest: { bg: 'bg-gray-500/10', text: 'text-gray-400', border: 'border-gray-500/20' },
    race: { bg: 'bg-amber-500/20', text: 'text-amber-300', border: 'border-amber-500/40' },
    default: { bg: 'bg-blue-500/15', text: 'text-blue-300', border: 'border-blue-500/25' },
};

function extractTypeStr(val: unknown): string {
    if (!val) return '';
    if (typeof val === 'string') return val;
    if (typeof val === 'object' && val !== null) {
        const obj = val as Record<string, unknown>;
        return String(obj.typeKey || obj.type || obj.name || '');
    }
    return String(val);
}

const DONE_SPORT_COLORS: Record<string, { bg: string; text: string; border: string }> = {
    strength: { bg: 'bg-red-500/10', text: 'text-red-400/80', border: 'border-red-500/20' },
    running: { bg: 'bg-green-500/10', text: 'text-green-400/80', border: 'border-green-500/20' },
    cycling: { bg: 'bg-amber-500/10', text: 'text-amber-400/80', border: 'border-amber-500/20' },
    swimming: { bg: 'bg-cyan-500/10', text: 'text-cyan-400/80', border: 'border-cyan-500/20' },
    yoga: { bg: 'bg-purple-500/10', text: 'text-purple-400/80', border: 'border-purple-500/20' },
    default: { bg: 'bg-emerald-500/10', text: 'text-emerald-400/80', border: 'border-emerald-500/20' },
};

function getDoneSportStyle(sport: string): typeof DONE_SPORT_COLORS.default {
    const key = sport.toLowerCase();
    if (key.includes('strength') || key.includes('weight') || key.includes('gym')) return DONE_SPORT_COLORS.strength;
    if (key.includes('run') || key.includes('trail')) return DONE_SPORT_COLORS.running;
    if (key.includes('cycl') || key.includes('bike')) return DONE_SPORT_COLORS.cycling;
    if (key.includes('swim')) return DONE_SPORT_COLORS.swimming;
    if (key.includes('yoga') || key.includes('stretch')) return DONE_SPORT_COLORS.yoga;
    return DONE_SPORT_COLORS.default;
}

function getSportStyle(sport?: string, type?: string, isRace?: boolean): typeof SPORT_COLORS.default {
    if (isRace) return SPORT_COLORS.race;
    const key = (sport || type || '').toLowerCase();
    if (key.includes('strength') || key.includes('weight') || key.includes('gym')) return SPORT_COLORS.strength;
    if (key.includes('run')) return SPORT_COLORS.running;
    if (key.includes('cycl') || key.includes('bike')) return SPORT_COLORS.cycling;
    if (key.includes('swim')) return SPORT_COLORS.swimming;
    if (key.includes('yoga') || key.includes('stretch')) return SPORT_COLORS.yoga;
    if (key.includes('rest')) return SPORT_COLORS.rest;
    return SPORT_COLORS.default;
}

function isRunWorkout(w: PlannedWorkout): boolean {
    const sport = (w.sport || w.type || '').toLowerCase();
    return sport.includes('run') || sport.includes('trail');
}

type WorkoutStep = { label: string; detail?: string; indent?: number };

function fmtPace(v: number) { const m = Math.floor(v / 60); const s = Math.round(v % 60); return `${m}:${String(s).padStart(2, '0')}`; }

function fmtDuration(secs: number): string {
    const mins = Math.floor(secs / 60);
    const s = Math.round(secs % 60);
    return s > 0 ? `${mins}:${String(s).padStart(2, '0')}` : `${mins} min`;
}

function formatStepDetail(step: any): string | undefined {
    const endCond = step?.endCondition;
    const condType = endCond?.conditionTypeKey || '';
    const condVal = step?.endConditionValue ?? endCond?.value;
    let detail: string | undefined;

    if (condType === 'distance' && condVal) {
        detail = `${(condVal / 1000).toFixed(2)} km`;
    } else if (condType === 'time' && condVal) {
        detail = fmtDuration(condVal);
    } else if (condType === 'lap.button') {
        detail = 'Lap button';
    }

    const target = step?.targetType?.workoutTargetTypeKey;
    if (target && target !== 'no.target') {
        const from = step?.targetValueOne;
        const to = step?.targetValueTwo;
        if (target === 'pace.zone' && from && to) {
            detail = (detail ? detail + ' · ' : '') + `${fmtPace(from)}-${fmtPace(to)}/km`;
        } else if (target === 'heart.rate.zone' && from && to) {
            detail = (detail ? detail + ' · ' : '') + `${Math.round(from)}-${Math.round(to)} bpm`;
        }
    }
    return detail;
}

function stepLabel(step: any): string {
    const stepType = step?.stepType?.stepTypeKey || '';
    const exName = step?.exerciseName
        ? String(step.exerciseName).replace(/_/g, ' ').toLowerCase().replace(/\b\w/g, (c: string) => c.toUpperCase())
        : null;
    if (stepType === 'warmup') return exName || 'Warm Up';
    if (stepType === 'cooldown') return exName || 'Cool Down';
    if (stepType === 'interval') return exName || 'Interval';
    if (stepType === 'recovery') return exName || 'Recovery';
    if (stepType === 'rest') return exName || 'Rest';
    return exName || stepType || 'Step';
}

function processSteps(stepsArr: any[], into: WorkoutStep[], indent: number, repeatPrefix?: string) {
    for (const step of stepsArr) {
        const stepType = step?.stepType?.stepTypeKey || step?.type || '';

        if (step?.type === 'RepeatGroupDTO' || stepType === 'repeat') {
            const reps = step.numberOfIterations || step.endConditionValue || 1;
            const innerSteps = step.workoutSteps;
            if (!Array.isArray(innerSteps)) continue;

            // Check if this is a strength repeat (exercises with weight/reps) or a cardio repeat (intervals)
            const isStrength = innerSteps.some((s: any) =>
                s?.weightValue != null || s?.repValue != null ||
                (s?.endCondition?.conditionTypeKey === 'reps')
            );

            if (isStrength) {
                for (const inner of innerSteps) {
                    const innerType = inner?.stepType?.stepTypeKey || '';
                    if (innerType === 'rest') continue;
                    const rawName = inner?.exerciseName || inner?.category || inner?.description || 'Exercise';
                    const exName = typeof rawName === 'string'
                        ? rawName.replace(/_/g, ' ').toLowerCase().replace(/\b\w/g, (c: string) => c.toUpperCase())
                        : typeof rawName === 'object' ? (rawName as any)?.exerciseName || 'Exercise' : String(rawName);
                    const weight = typeof inner?.weightValue === 'number' ? inner.weightValue : inner?.weightValue?.value;
                    const repCount = (inner?.endCondition?.conditionTypeKey === 'reps' ? inner?.endConditionValue : null)
                        || inner?.repValue?.value || inner?.repValue;
                    let detail = `${reps}x`;
                    if (repCount) detail += ` ${Math.round(repCount)} reps`;
                    if (weight) detail += ` @ ${weight}kg`;
                    into.push({ label: exName, detail, indent });
                }
            } else {
                // Cardio repeat group — show header then expand inner steps
                const prefix = repeatPrefix ? `${repeatPrefix}${Math.round(reps)} × ` : `${Math.round(reps)} × `;
                into.push({ label: `Repeat`, detail: `${prefix}`, indent });
                processSteps(innerSteps, into, indent + 1);
            }
            continue;
        }

        if (stepType === 'rest') {
            // Show rest steps inside repeat groups with their duration
            const detail = formatStepDetail(step);
            if (detail) {
                into.push({ label: 'Rest', detail, indent });
            }
            continue;
        }

        into.push({ label: stepLabel(step), detail: formatStepDetail(step), indent });
    }
}

function extractWorkoutSteps(w: PlannedWorkout): WorkoutStep[] {
    const steps: WorkoutStep[] = [];

    const sources = [w.adaptive_details, w.workout_detail, w as any];
    for (const src of sources) {
        const segments = src?.workoutSegments;
        if (!Array.isArray(segments)) continue;

        for (const seg of segments) {
            const wSteps = seg?.workoutSteps;
            if (!Array.isArray(wSteps)) continue;
            processSteps(wSteps, steps, 0);
        }
        if (steps.length > 0) break;
    }

    return steps;
}

/** Format a Date as YYYY-MM-DD in local time (avoids UTC shift from toISOString) */
function toLocalDateKey(d: Date): string {
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, '0');
    const day = String(d.getDate()).padStart(2, '0');
    return `${y}-${m}-${day}`;
}

export default function WeeklyCalendar({ upcomingWorkouts, todayPlanned, completedCount, weekActivities = [] }: WeeklyCalendarProps) {
    const [expandedDay, setExpandedDay] = useState<string | null>(null);

    const today = new Date();
    today.setHours(0, 0, 0, 0);

    // Build 7-day range: 3 past + today + 3 future
    const days: { date: Date; label: string; shortDay: string; isToday: boolean; isPast: boolean }[] = [];
    for (let i = -3; i <= 3; i++) {
        const d = new Date(today);
        d.setDate(today.getDate() + i);
        days.push({
            date: d,
            label: d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' }),
            shortDay: i === 0 ? 'Today' : d.toLocaleDateString(undefined, { weekday: 'short' }),
            isToday: i === 0,
            isPast: i < 0,
        });
    }

    // Group planned workouts by date string (YYYY-MM-DD)
    const workoutsByDate: Record<string, PlannedWorkout[]> = {};
    const todayKey = toLocalDateKey(today);
    workoutsByDate[todayKey] = [...todayPlanned];

    for (const w of upcomingWorkouts || []) {
        if (!w || !w.date) continue;
        const key = w.date.substring(0, 10);
        if (!workoutsByDate[key]) workoutsByDate[key] = [];
        if (key === todayKey) {
            const isDuplicate = workoutsByDate[key].some(
                (existing) => (existing.title || existing.name) === (w.title || w.name)
            );
            if (!isDuplicate) workoutsByDate[key].push(w);
        } else {
            workoutsByDate[key].push(w);
        }
    }

    // Group completed activities by date
    type DoneActivity = {
        label: string; sport: string; duration?: number; distance?: number;
        averageHR?: number; maxHR?: number; calories?: number;
        averageSpeed?: number; elevationGain?: number; avgPower?: number;
    };
    const doneByDate: Record<string, DoneActivity[]> = {};
    for (const act of weekActivities || []) {
        if (!act || !act.startTimeLocal) continue;
        const key = act.startTimeLocal.substring(0, 10);
        if (!doneByDate[key]) doneByDate[key] = [];
        const actName = act.name || extractTypeStr(act.activity_type) || extractTypeStr(act.type) || 'Activity';
        const actSport = extractTypeStr(act.sport) || extractTypeStr(act.activity_type) || extractTypeStr(act.type) || '';
        doneByDate[key].push({
            label: actName, sport: actSport, duration: act.duration, distance: act.distance,
            averageHR: act.averageHR, maxHR: act.maxHR, calories: act.calories,
            averageSpeed: act.averageSpeed, elevationGain: act.elevationGain, avgPower: act.avgPower,
        });
    }

    return (
        <div className="glass-panel p-3 bento-full section-reveal overflow-visible">
            <div className="flex items-center justify-between mb-2">
                <h3 className="text-xs font-semibold tracking-wider text-gray-300 uppercase">This Week</h3>
                {completedCount > 0 && (
                    <span className="text-[10px] text-emerald-400 font-medium bg-emerald-500/10 px-2 py-0.5 rounded-full border border-emerald-500/20">
                        {completedCount} done today
                    </span>
                )}
            </div>

            <div className="space-y-1">
                {days.map((day) => {
                    const dateKey = toLocalDateKey(day.date);
                    const planned = workoutsByDate[dateKey] || [];
                    const done = doneByDate[dateKey] || [];
                    const hasItems = planned.length > 0 || done.length > 0;
                    const isExpanded = expandedDay === dateKey;

                    return (
                        <div
                            key={dateKey}
                            className={`rounded-xl border transition-all ${
                                day.isToday
                                    ? 'border-red-500/30 bg-red-500/[0.04]'
                                    : day.isPast
                                    ? 'border-white/[0.03] bg-white/[0.01] opacity-75'
                                    : 'border-white/[0.05] bg-white/[0.02]'
                            }`}
                        >
                            {/* Day header row - always visible, clickable to expand */}
                            <button
                                className="w-full flex items-center gap-3 px-3 py-2 text-left"
                                onClick={() => setExpandedDay(isExpanded ? null : dateKey)}
                            >
                                <div className="flex items-center gap-2 min-w-[100px]">
                                    <span className={`text-xs font-semibold w-10 ${day.isToday ? 'text-red-400' : day.isPast ? 'text-gray-600' : 'text-gray-400'}`}>
                                        {day.shortDay}
                                    </span>
                                    <span className={`text-[11px] ${day.isToday ? 'text-gray-300' : 'text-gray-600'}`}>
                                        {day.label}
                                    </span>
                                </div>

                                {/* Inline summary badges */}
                                <div className="flex-1 flex items-center gap-1.5 overflow-hidden">
                                    {done.map((d, i) => {
                                        const doneStyle = getDoneSportStyle(d.sport);
                                        return (
                                            <span key={`d-${i}`} className={`inline-flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded-md ${doneStyle.bg} ${doneStyle.text} border ${doneStyle.border} whitespace-nowrap`}>
                                                <span className="opacity-70">&#10003;</span> {d.label.length > 20 ? d.label.slice(0, 20) + '...' : d.label}
                                            </span>
                                        );
                                    })}
                                    {planned.map((w, i) => {
                                        const style = getSportStyle(w.sport, w.type, w.is_race);
                                        const shortLabel = w.adaptive_details?.workoutName || w.title || w.name || w.sport || w.type || 'Workout';
                                        return (
                                            <span key={`p-${i}`} className={`inline-flex items-center text-[10px] px-1.5 py-0.5 rounded-md ${style.bg} ${style.text} border ${style.border} whitespace-nowrap`}>
                                                {shortLabel.length > 20 ? shortLabel.slice(0, 20) + '...' : shortLabel}
                                            </span>
                                        );
                                    })}
                                    {!hasItems && (
                                        <span className="text-[10px] text-gray-700 italic">Rest day</span>
                                    )}
                                </div>

                                {/* Expand indicator */}
                                {hasItems && (
                                    <svg
                                        xmlns="http://www.w3.org/2000/svg"
                                        width="12" height="12"
                                        viewBox="0 0 24 24"
                                        fill="none"
                                        stroke="currentColor"
                                        strokeWidth="2"
                                        strokeLinecap="round"
                                        strokeLinejoin="round"
                                        className={`text-gray-600 transition-transform duration-200 flex-shrink-0 ${isExpanded ? 'rotate-180' : ''}`}
                                    >
                                        <polyline points="6 9 12 15 18 9" />
                                    </svg>
                                )}
                            </button>

                            {/* Expanded detail panel */}
                            {isExpanded && hasItems && (
                                <div className="px-3 pb-3 space-y-2 border-t border-white/[0.04] pt-2 mx-1">
                                    {/* Completed activities */}
                                    {done.map((d, i) => {
                                        const doneStyle = getDoneSportStyle(d.sport);
                                        return (
                                            <div key={`done-${i}`} className={`rounded-lg p-2.5 border ${doneStyle.border} ${doneStyle.bg}`}>
                                                <div className="flex items-center justify-between">
                                                    <div className="flex items-center gap-2">
                                                        <span className="text-emerald-400 text-[10px]">&#10003;</span>
                                                        <span className={`text-xs font-medium ${doneStyle.text}`}>{d.label}</span>
                                                    </div>
                                                    <span className="text-[10px] text-gray-600">{d.sport}</span>
                                                </div>
                                                <div className="flex items-center flex-wrap gap-x-3 gap-y-0.5 mt-1.5 text-[11px] text-gray-400">
                                                    {d.duration != null && <span>{(d.duration / 60).toFixed(0)} min</span>}
                                                    {d.distance != null && d.distance > 0 && <span>{(d.distance / 1000).toFixed(1)} km</span>}
                                                    {d.averageHR != null && <span>{Math.round(d.averageHR)} avg / {d.maxHR ? Math.round(d.maxHR) : '--'} max bpm</span>}
                                                    {d.calories != null && d.calories > 0 && <span>{Math.round(d.calories)} kcal</span>}
                                                    {d.averageSpeed != null && d.averageSpeed > 0 && d.distance != null && d.distance > 0 && (
                                                        <span>
                                                            {(() => {
                                                                // averageSpeed is m/s — convert to pace min/km for running, km/h for cycling
                                                                const isRun = d.sport.toLowerCase().includes('run') || d.sport.toLowerCase().includes('trail');
                                                                if (isRun) {
                                                                    const paceS = 1000 / d.averageSpeed!;
                                                                    const m = Math.floor(paceS / 60);
                                                                    const s = Math.round(paceS % 60);
                                                                    return `${m}:${String(s).padStart(2, '0')}/km`;
                                                                }
                                                                return `${(d.averageSpeed! * 3.6).toFixed(1)} km/h`;
                                                            })()}
                                                        </span>
                                                    )}
                                                    {d.elevationGain != null && d.elevationGain > 0 && <span>{Math.round(d.elevationGain)} m elev</span>}
                                                    {d.avgPower != null && d.avgPower > 0 && <span>{Math.round(d.avgPower)} W avg</span>}
                                                </div>
                                            </div>
                                        );
                                    })}

                                    {/* Planned workouts with full detail */}
                                    {planned.map((w, i) => {
                                        const style = getSportStyle(w.sport, w.type, w.is_race);
                                        const fullName = w.adaptive_details?.workoutName || w.title || w.name || 'Workout';
                                        const desc = w.adaptive_details?.description || w.description;
                                        const distanceKm = (w.distance || w.adaptive_details?.estimatedDistanceInMeters)
                                            ? ((w.distance || w.adaptive_details?.estimatedDistanceInMeters) / 1000).toFixed(1) : null;
                                        const steps = extractWorkoutSteps(w);
                                        const isRun = isRunWorkout(w);

                                        return (
                                            <div key={`plan-${i}`} className={`rounded-lg p-2.5 border ${style.border} ${style.bg}`}>
                                                <div className="flex items-center justify-between">
                                                    <span className={`text-xs font-medium ${style.text}`}>
                                                        {w.adaptive_details?.workoutName ? `✨ ${fullName}` : fullName}
                                                    </span>
                                                    <span className="text-[10px] text-gray-600">{w.sport || w.type || 'Workout'}</span>
                                                </div>
                                                {desc && <div className="text-[11px] text-gray-500 mt-1 italic line-clamp-2">{desc}</div>}
                                                <div className="flex items-center gap-3 mt-1.5 text-[11px] text-gray-400">
                                                    {w.duration != null && <span>{w.duration.toFixed(0)} min</span>}
                                                    {distanceKm && <span>{distanceKm} km</span>}
                                                </div>

                                                {/* Workout steps */}
                                                {steps.length > 0 && (
                                                    <div className="mt-2 pt-2 border-t border-white/[0.05] space-y-0.5">
                                                        {steps.map((s, si) => (
                                                            <div key={si} className="flex items-baseline justify-between gap-2 text-[11px]" style={s.indent ? { paddingLeft: `${s.indent * 12}px` } : undefined}>
                                                                <span className={`truncate ${s.indent ? 'text-gray-400' : 'text-gray-300'}`}>{s.label}</span>
                                                                {s.detail && <span className="text-gray-500 whitespace-nowrap flex-shrink-0">{s.detail}</span>}
                                                            </div>
                                                        ))}
                                                    </div>
                                                )}

                                                {isRun && !day.isPast && (
                                                    <div className="mt-2 pt-2 border-t border-white/[0.05]">
                                                        <CreateCourseButton workout={w} />
                                                    </div>
                                                )}
                                            </div>
                                        );
                                    })}
                                </div>
                            )}
                        </div>
                    );
                })}
            </div>
        </div>
    );
}
