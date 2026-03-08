'use client';

import React, { useState, useEffect, useRef } from 'react';
import ReactMarkdown from 'react-markdown';

type ChatMessage = {
    role: string;
    content: string;
    created_at?: number;
};

function stripNode<T extends { node?: unknown }>(props: T): Omit<T, 'node'> {
    const { node, ...rest } = props;
    void node;
    return rest;
}

type ChatProps = {
    embedded?: boolean;
};

export default function Chat({ embedded = false }: ChatProps) {
    const [messages, setMessages] = useState<ChatMessage[]>([]);
    const [input, setInput] = useState('');
    const [loading, setLoading] = useState(false);
    const chatContainerRef = useRef<HTMLDivElement>(null);

    const fetchChat = async () => {
        try {
            const res = await fetch('/api/chat');
            if (res.ok) {
                const data = await res.json();
                setMessages(data);
            }
        } catch (e) {
            console.error(e);
        }
    };

    useEffect(() => {
        fetchChat();
    }, []);

    useEffect(() => {
        if (chatContainerRef.current) {
            chatContainerRef.current.scrollTo({
                top: chatContainerRef.current.scrollHeight,
                behavior: 'smooth'
            });
        }
    }, [messages]);

    const handleSend = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!input.trim() || loading) return;

        const userMessage = input.trim();
        setInput('');
        setMessages((prev) => [...prev, { role: 'user', content: userMessage, created_at: Math.floor(Date.now() / 1000) }]);
        setLoading(true);

        try {
            const res = await fetch('/api/chat', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ content: userMessage }),
            });
            if (res.ok) {
                await fetchChat();
            }
        } catch (err) {
            console.error(err);
        } finally {
            setLoading(false);
        }
    };

    const containerClass = embedded
        ? 'flex flex-col h-full'
        : 'glass-panel-elevated p-6 flex flex-col h-[600px] border border-white/8 group relative overflow-hidden';

    return (
        <div className={containerClass}>
            {/* Ambient glow - only in standalone mode */}
            {!embedded && (
                <>
                    <div className="ambient-glow bg-indigo-500 -left-16 -top-16" style={{ width: '180px', height: '180px', animation: 'pulseGlow 4s ease-in-out infinite' }} />
                    <div className="ambient-glow bg-purple-500 -right-12 -bottom-12" style={{ width: '140px', height: '140px', animation: 'pulseGlow 5s ease-in-out infinite 1s' }} />
                </>
            )}

            {!embedded && (
                <h3 className="text-gray-400 font-medium tracking-wide mb-4 z-10 flex items-center justify-between text-xs uppercase">
                    <span className="flex items-center gap-2">
                        <span className="w-2 h-2 rounded-full bg-blue-500 shadow-[0_0_8px_rgba(79,140,255,0.6)]" />
                        ✦ AI Coach Chat
                    </span>
                    <button onClick={fetchChat} className="text-xs bg-white/5 hover:bg-white/10 px-3 py-1.5 rounded-lg border border-white/8 transition-all hover:border-white/15">Refresh</button>
                </h3>
            )}

            <div ref={chatContainerRef} className={`flex-1 overflow-y-auto space-y-4 pr-2 z-10 scrollbar-thin scrollbar-thumb-white/10 scrollbar-track-transparent ${embedded ? 'p-4' : 'mb-4'}`}>
                {messages.length === 0 && !loading && (
                    <div className="flex flex-col items-center justify-center h-full text-center text-gray-500 space-y-2 mt-10">
                        <svg className="w-12 h-12 text-blue-500/40 mb-2" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
                        </svg>
                        <p>No messages yet.</p>
                        <p className="text-sm">Say hi to your AI Coach to get started!</p>
                    </div>
                )}
                {messages.map((msg, idx) => (
                    <div key={idx} className={`p-4 rounded-2xl max-w-[90%] backdrop-blur-sm transition-all ${msg.role === 'user'
                        ? 'bg-blue-500/15 text-blue-100 ml-auto border border-blue-500/25 shadow-[0_4px_20px_rgba(79,140,255,0.1)]'
                        : 'bg-white/[0.03] text-gray-300 border border-white/8 shadow-[0_4px_20px_rgba(0,0,0,0.15)]'
                        }`}>
                        <div className="text-xs text-gray-500 mb-2 tracking-wider flex justify-between items-center whitespace-nowrap gap-4">
                            <span className="uppercase overflow-hidden text-ellipsis">{msg.role === 'user' ? 'You' : 'Coach Gemini'}</span>
                            {msg.created_at && (
                                <span className="text-[10px] opacity-70 normal-case">
                                    {new Date(msg.created_at * 1000).toLocaleString(undefined, {
                                        year: 'numeric',
                                        month: 'numeric',
                                        day: 'numeric',
                                        hour: '2-digit',
                                        minute: '2-digit',
                                    })}
                                </span>
                            )}
                        </div>
                        <div className="text-sm leading-relaxed whitespace-pre-wrap break-words overflow-hidden">
                            <ReactMarkdown
                                components={{
                                    p: (props) => <p className="mb-2 last:mb-0" {...stripNode(props)} />,
                                    strong: (props) => <strong className="font-semibold text-white/90" {...stripNode(props)} />,
                                    ul: (props) => <ul className="list-disc pl-5 mb-2 space-y-1" {...stripNode(props)} />,
                                    ol: (props) => <ol className="list-decimal pl-5 mb-2 space-y-1" {...stripNode(props)} />,
                                    li: (props) => <li className="pl-1" {...stripNode(props)} />,
                                    h1: (props) => <h1 className="text-lg font-bold text-white mb-2 mt-4" {...stripNode(props)} />,
                                    h2: (props) => <h2 className="text-base font-bold text-white mb-2 mt-3" {...stripNode(props)} />,
                                    h3: (props) => <h3 className="text-sm font-bold text-white mb-2 mt-2" {...stripNode(props)} />,
                                    pre: (props) => <pre className="bg-black/60 p-3 rounded-xl mb-2 overflow-x-auto text-xs font-mono border border-white/5" {...stripNode(props)} />,
                                    code: (props) => {
                                        const { className, ...rest } = stripNode(props);
                                        const match = /language-(\w+)/.exec(className || '')
                                        return !match ? (
                                            <code className="bg-white/10 px-1.5 py-0.5 rounded text-xs break-words" {...rest} />
                                        ) : (
                                            <code className={className} {...rest} />
                                        )
                                    }
                                }}
                            >
                                {msg.content}
                            </ReactMarkdown>
                        </div>
                    </div>
                ))}
                {loading && (
                    <div className="p-4 rounded-2xl max-w-[90%] bg-white/[0.03] text-gray-400 border border-white/8">
                        <div className="flex items-center gap-1.5">
                            <span className="inline-block w-2 h-2 bg-blue-500 rounded-full animate-bounce shadow-[0_0_6px_rgba(79,140,255,0.5)]"></span>
                            <span className="inline-block w-2 h-2 bg-blue-500 rounded-full animate-bounce shadow-[0_0_6px_rgba(79,140,255,0.5)]" style={{ animationDelay: '0.15s' }}></span>
                            <span className="inline-block w-2 h-2 bg-blue-500 rounded-full animate-bounce shadow-[0_0_6px_rgba(79,140,255,0.5)]" style={{ animationDelay: '0.3s' }}></span>
                        </div>
                    </div>
                )}
            </div>

            <form onSubmit={handleSend} className={`flex gap-3 relative z-10 mt-auto ${embedded ? 'p-4 pt-0' : ''}`}>
                <input
                    type="text"
                    value={input}
                    onChange={(e) => setInput(e.target.value)}
                    placeholder="Ask Coach about your plan..."
                    className="flex-1 bg-black/30 border border-white/8 rounded-2xl px-5 py-3 text-white focus:outline-none focus:border-blue-500/40 focus:shadow-[0_0_0_3px_rgba(79,140,255,0.1)] transition-all placeholder:text-gray-600 backdrop-blur-sm"
                    disabled={loading}
                />
                <button
                    type="submit"
                    disabled={loading || !input.trim()}
                    className="bg-blue-500/15 hover:bg-blue-500/30 text-blue-300 font-medium py-3 px-8 rounded-2xl transition-all border border-blue-500/25 hover:border-blue-500/40 disabled:opacity-40 disabled:cursor-not-allowed hover:shadow-[0_0_20px_rgba(79,140,255,0.15)]"
                >
                    Send
                </button>
            </form>
        </div>
    );
}
