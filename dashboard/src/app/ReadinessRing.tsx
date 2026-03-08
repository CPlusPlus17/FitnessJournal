import React from 'react';

type ReadinessRingProps = {
    value: number | null;
    label?: string;
};

export default function ReadinessRing({ value, label }: ReadinessRingProps) {
    const displayValue = value ?? 0;
    const circumference = 2 * Math.PI * 45; // r=45
    const progress = Math.min(Math.max(displayValue, 0), 100) / 100;
    const dashTarget = circumference * (1 - progress);

    // Color based on value
    let strokeColor = '#ff6b6b'; // red (low)
    let glowColor = 'rgba(255, 107, 107, 0.3)';
    let statusText = 'REST DAY';
    if (displayValue >= 70) {
        strokeColor = '#34d399'; // green (high)
        glowColor = 'rgba(52, 211, 153, 0.3)';
        statusText = 'READY TO TRAIN';
    } else if (displayValue >= 40) {
        strokeColor = '#f59e0b'; // amber (moderate)
        glowColor = 'rgba(245, 158, 11, 0.3)';
        statusText = 'MODERATE';
    }

    if (value === null) {
        statusText = 'NO DATA';
    }

    return (
        <div className="glass-panel hover-lift p-4 flex flex-col items-center justify-center gap-1 group relative overflow-hidden bento-hero h-full">
            {/* Ambient glow behind ring */}
            <div
                className="absolute rounded-full pointer-events-none"
                style={{
                    width: '140px',
                    height: '140px',
                    top: '50%',
                    left: '50%',
                    transform: 'translate(-50%, -50%)',
                    background: glowColor,
                    filter: 'blur(50px)',
                    opacity: 0.35,
                    transition: 'background 0.5s ease, opacity 0.5s ease',
                }}
            />

            <div className="relative z-10">
                <svg width="80" height="80" viewBox="0 0 100 100">
                    <defs>
                        <linearGradient id="ringGradient" x1="0%" y1="0%" x2="100%" y2="100%">
                            <stop offset="0%" stopColor={strokeColor} stopOpacity="1" />
                            <stop offset="100%" stopColor={strokeColor} stopOpacity="0.6" />
                        </linearGradient>
                        <filter id="ringGlow">
                            <feGaussianBlur stdDeviation="2" result="blur" />
                            <feMerge>
                                <feMergeNode in="blur" />
                                <feMergeNode in="SourceGraphic" />
                            </feMerge>
                        </filter>
                    </defs>

                    <circle cx="50" cy="50" r="45" className="ring-gauge-track" />

                    {value !== null && (
                        <circle
                            cx="50"
                            cy="50"
                            r="45"
                            className="ring-gauge-value"
                            stroke="url(#ringGradient)"
                            filter="url(#ringGlow)"
                            strokeDasharray={circumference}
                            style={{
                                '--dash-full': circumference,
                                '--dash-target': dashTarget,
                            } as React.CSSProperties}
                        />
                    )}

                    <text
                        x="50"
                        y="44"
                        textAnchor="middle"
                        dominantBaseline="central"
                        className="font-extrabold"
                        fill="white"
                        fontSize="26"
                        style={{ fontFamily: 'var(--font-sans)' }}
                    >
                        {value !== null ? displayValue : '--'}
                    </text>

                    <text
                        x="50"
                        y="62"
                        textAnchor="middle"
                        dominantBaseline="central"
                        fill="rgba(255,255,255,0.5)"
                        fontSize="6"
                        letterSpacing="0.1em"
                        style={{ fontFamily: 'var(--font-sans)' }}
                    >
                        {statusText}
                    </text>
                </svg>
            </div>

            <h3 className="text-gray-400 font-medium tracking-wide text-[10px] uppercase z-10">
                {label || 'Training Readiness'}
            </h3>
        </div>
    );
}
