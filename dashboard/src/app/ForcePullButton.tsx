'use client';

import React, { useState } from 'react';
import { useRouter } from 'next/navigation';

export default function ForcePullButton() {
    const [loading, setLoading] = useState(false);
    const router = useRouter();

    const handlePull = async () => {
        setLoading(true);
        try {
            await fetch('/api/force-pull', { method: 'POST' });
            router.refresh();
        } catch (err) {
            console.error(err);
        }
        setLoading(false);
    }

    return (
        <button
            onClick={handlePull}
            disabled={loading}
            className="px-4 py-2 bg-white/10 text-white rounded-md hover:bg-white/20 transition backdrop-blur-md border border-white/20 flex items-center gap-2 disabled:opacity-50 h-[42px]"
        >
            {loading ? (
                <>
                    <span className="w-4 h-4 rounded-full border-2 border-white border-t-transparent animate-spin"></span>
                    Syncing...
                </>
            ) : (
                <>
                    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M21 2v6h-6M3 12a9 9 0 1 0 2-7.7L2 6" /><path d="M21 8A9 9 0 0 0 4.3 3.3L8 2" /></svg>
                    Force Sync
                </>
            )}
        </button>
    )
}
