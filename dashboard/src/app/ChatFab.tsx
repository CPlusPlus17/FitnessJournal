'use client';

import React, { useState, useCallback, useEffect } from 'react';
import Chat from './Chat';

export default function ChatFab() {
    const [isOpen, setIsOpen] = useState(false);
    const [isClosing, setIsClosing] = useState(false);

    const handleOpen = useCallback(() => {
        setIsOpen(true);
        setIsClosing(false);
    }, []);

    const handleClose = useCallback(() => {
        setIsClosing(true);
        setTimeout(() => {
            setIsOpen(false);
            setIsClosing(false);
        }, 300);
    }, []);

    // Close on Escape key
    useEffect(() => {
        if (!isOpen) return;
        const handleKey = (e: KeyboardEvent) => {
            if (e.key === 'Escape') handleClose();
        };
        window.addEventListener('keydown', handleKey);
        return () => window.removeEventListener('keydown', handleKey);
    }, [isOpen, handleClose]);

    return (
        <>
            {/* FAB Button */}
            <button
                onClick={isOpen ? handleClose : handleOpen}
                className="fab-button"
                aria-label={isOpen ? 'Close chat' : 'Open AI Coach Chat'}
            >
                {isOpen ? (
                    <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                        <path d="M18 6 6 18" /><path d="m6 6 12 12" />
                    </svg>
                ) : (
                    <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                        <path d="M7.9 20A9 9 0 1 0 4 16.1L2 22Z" />
                    </svg>
                )}
            </button>

            {/* Overlay + Panel */}
            {isOpen && (
                <>
                    <div className="slide-panel-overlay" onClick={handleClose} />
                    <div className={`slide-panel ${isClosing ? 'is-closing' : 'is-open'}`}>
                        <div className="flex flex-col h-full">
                            {/* Panel header */}
                            <div className="flex items-center justify-between px-5 py-4 border-b border-white/8">
                                <div className="flex items-center gap-2">
                                    <span className="w-2 h-2 rounded-full bg-blue-500 shadow-[0_0_8px_rgba(79,140,255,0.6)]" />
                                    <span className="text-sm font-semibold text-white tracking-wide">✦ AI Coach</span>
                                </div>
                                <button
                                    onClick={handleClose}
                                    className="w-8 h-8 flex items-center justify-center rounded-lg hover:bg-white/8 transition-colors text-gray-400 hover:text-white"
                                >
                                    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                        <path d="M18 6 6 18" /><path d="m6 6 12 12" />
                                    </svg>
                                </button>
                            </div>

                            {/* Chat component fills remaining space */}
                            <div className="flex-1 overflow-hidden">
                                <Chat embedded={true} />
                            </div>
                        </div>
                    </div>
                </>
            )}
        </>
    );
}
