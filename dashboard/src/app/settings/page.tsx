"use client";

import React, { useEffect, useState } from 'react';
import Link from 'next/link';

type Profile = {
    goals: string[];
    constraints: string[];
    available_equipment: string[];
};

type ProfilesData = {
    active_profile: string;
    profiles: Record<string, Profile>;
};

export default function SettingsPage() {
    const [data, setData] = useState<ProfilesData | null>(null);
    const [loading, setLoading] = useState(true);
    const [saving, setSaving] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [saveNotice, setSaveNotice] = useState<{ type: 'success' | 'error'; message: string } | null>(null);

    useEffect(() => {
        fetch('/api/profiles')
            .then(res => {
                if (!res.ok) throw new Error('Failed to load profiles');
                return res.json();
            })
            .then(json => {
                setData(json);
                setLoading(false);
            })
            .catch(err => {
                setError(err.message);
                setLoading(false);
            });
    }, []);

    const handleSave = async () => {
        if (!data) return;
        setSaving(true);
        setError(null);
        setSaveNotice(null);
        try {
            const res = await fetch('/api/profiles', {
                method: 'PUT',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(data),
            });
            if (!res.ok) {
                let message = 'Failed to save profiles';
                try {
                    const response = await res.json() as { message?: string };
                    if (response?.message) {
                        message = response.message;
                    }
                } catch {
                    // Keep default message if body is not JSON.
                }
                throw new Error(message);
            }
            setSaveNotice({ type: 'success', message: 'Settings saved successfully.' });
        } catch (err: unknown) {
            setSaveNotice({
                type: 'error',
                message: err instanceof Error ? err.message : 'Failed to save profiles',
            });
        } finally {
            setSaving(false);
        }
    };

    if (loading) return <div className="p-8 text-white min-h-screen">Loading settings...</div>;
    if (error && !data) return <div className="p-8 text-red-500 min-h-screen">Error: {error}</div>;
    if (!data) return null;

    const activeProfileData = data.profiles[data.active_profile] || {
        goals: [],
        constraints: [],
        available_equipment: [],
    };

    return (
        <main className="min-h-screen p-8 md:p-24 selection:bg-red-500 selection:text-white pb-32">
            <div className="max-w-4xl mx-auto space-y-12">
                <header className="flex flex-col md:flex-row md:items-start md:justify-between space-y-4 md:space-y-0">
                    <div className="space-y-4">
                        <h1 className="text-4xl md:text-6xl font-extrabold tracking-tight text-white">
                            Configuration
                        </h1>
                        <p className="text-gray-400 text-lg md:text-xl">
                            Manage equipment and constraints for the AI coach.
                        </p>
                    </div>
                    <div>
                        <Link href="/" className="px-4 py-2 bg-white/10 text-white rounded-md hover:bg-white/20 transition backdrop-blur-md border border-white/20">
                            &larr; Back to Dashboard
                        </Link>
                    </div>
                </header>

                {error && <div className="p-4 bg-red-500/20 text-red-500 rounded-md border border-red-500/50">{error}</div>}
                {saveNotice && (
                    <div
                        className={`p-4 rounded-md border ${
                            saveNotice.type === 'success'
                                ? 'bg-emerald-500/20 text-emerald-300 border-emerald-500/50'
                                : 'bg-red-500/20 text-red-500 border-red-500/50'
                        }`}
                    >
                        {saveNotice.message}
                    </div>
                )}

                <div className="space-y-8 glass-panel p-8">
                    <div>
                        <label className="block text-sm font-medium text-gray-400 mb-2">Active Profile</label>
                        <select
                            value={data.active_profile}
                            onChange={(e) => setData({ ...data, active_profile: e.target.value })}
                            className="w-full bg-black/40 border border-white/10 rounded-md p-3 text-white focus:border-red-500 focus:outline-none backdrop-blur-md"
                        >
                            {Object.keys(data.profiles).map(profileName => (
                                <option key={profileName} value={profileName}>{profileName}</option>
                            ))}
                        </select>
                    </div>

                    <div className="space-y-4">
                        <h3 className="text-xl font-bold text-white">Equipment ({data.active_profile})</h3>
                        <textarea
                            className="w-full h-48 bg-black/40 border border-white/10 rounded-md p-3 text-gray-300 font-mono text-sm focus:border-red-500 focus:outline-none backdrop-blur-md"
                            value={activeProfileData?.available_equipment.join('\n') || ''}
                            onChange={(e) => {
                                const newEquip = e.target.value.split('\n');
                                setData({
                                    ...data,
                                    profiles: {
                                        ...data.profiles,
                                        [data.active_profile]: {
                                            ...activeProfileData,
                                            available_equipment: newEquip,
                                        }
                                    }
                                });
                            }}
                            placeholder="One piece of equipment per line"
                        />
                        <p className="text-xs text-gray-500">Edit the list of available equipment. Put each item on a new line.</p>
                    </div>

                    <div className="space-y-4">
                        <h3 className="text-xl font-bold text-white">Training Goals ({data.active_profile})</h3>
                        <textarea
                            className="w-full h-32 bg-black/40 border border-white/10 rounded-md p-3 text-gray-300 font-mono text-sm focus:border-red-500 focus:outline-none backdrop-blur-md"
                            value={activeProfileData?.goals?.join('\n') || ''}
                            onChange={(e) => {
                                const newGoals = e.target.value.split('\n');
                                setData({
                                    ...data,
                                    profiles: {
                                        ...data.profiles,
                                        [data.active_profile]: {
                                            ...activeProfileData,
                                            goals: newGoals,
                                        }
                                    }
                                });
                            }}
                            placeholder="One goal per line"
                        />
                        <p className="text-xs text-gray-500">Edit the primary training goals. Put each goal on a new line.</p>
                    </div>

                    <div className="space-y-4">
                        <h3 className="text-xl font-bold text-white">Constraints ({data.active_profile})</h3>
                        <textarea
                            className="w-full h-48 bg-black/40 border border-white/10 rounded-md p-3 text-gray-300 font-mono text-sm focus:border-red-500 focus:outline-none backdrop-blur-md"
                            value={activeProfileData?.constraints.join('\n') || ''}
                            onChange={(e) => {
                                const newConstraints = e.target.value.split('\n');
                                setData({
                                    ...data,
                                    profiles: {
                                        ...data.profiles,
                                        [data.active_profile]: {
                                            ...activeProfileData,
                                            constraints: newConstraints,
                                        }
                                    }
                                });
                            }}
                            placeholder="One constraint per line"
                        />
                        <p className="text-xs text-gray-500">Edit the constraints for training generation. Put each rule on a new line.</p>
                    </div>

                    <div className="pt-4 flex justify-end">
                        <button
                            onClick={handleSave}
                            disabled={saving}
                            className="px-6 py-3 bg-gradient-to-r from-red-500 to-orange-500 hover:from-red-600 hover:to-orange-600 text-white font-bold rounded-md transition-colors disabled:opacity-50 shadow-lg shadow-red-500/20"
                        >
                            {saving ? 'Saving...' : 'Save Configuration'}
                        </button>
                    </div>
                </div>
            </div>
        </main>
    );
}
