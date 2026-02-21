'use client';

import React, { useState, useEffect, useRef } from 'react';
import ReactMarkdown from 'react-markdown';

type ChatMessage = {
    role: string;
    content: string;
};

function stripNode<T extends { node?: unknown }>(props: T): Omit<T, 'node'> {
    const { node, ...rest } = props;
    void node;
    return rest;
}

export default function Chat() {
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
        setMessages((prev) => [...prev, { role: 'user', content: userMessage }]);
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


    return (
        <div className="glass-panel p-6 flex flex-col h-[600px] border border-white/10 group relative overflow-hidden">
            <div className="absolute -left-12 -top-12 w-32 h-32 rounded-full blur-2xl transition-all bg-indigo-500/10 group-hover:bg-indigo-500/20"></div>

            <h3 className="text-gray-400 font-medium tracking-wide mb-4 z-10 flex items-center justify-between">
                <span>AI COACH CHAT</span>
                <button onClick={fetchChat} className="text-xs bg-white/5 hover:bg-white/10 px-2 py-1 rounded border border-white/10 transition">Refresh</button>
            </h3>

            <div ref={chatContainerRef} className="flex-1 overflow-y-auto space-y-4 mb-4 pr-2 z-10 scrollbar-thin scrollbar-thumb-white/10 scrollbar-track-transparent">
                {messages.map((msg, idx) => (
                    <div key={idx} className={`p-4 rounded-2xl max-w-[90%] ${msg.role === 'user' ? 'bg-indigo-500/20 text-indigo-100 ml-auto border border-indigo-500/30' : 'bg-black/40 text-gray-300 border border-white/10'}`}>
                        <div className="text-xs text-gray-500 mb-2 uppercase tracking-wider">{msg.role === 'user' ? 'You' : 'Coach Gemini'}</div>
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
                                    pre: (props) => <pre className="bg-black/60 p-3 rounded mb-2 overflow-x-auto text-xs font-mono" {...stripNode(props)} />,
                                    code: (props) => {
                                        const { className, ...rest } = stripNode(props);
                                        // Next.js uses standard React types, we will just pass it down and add our own classes.
                                        // If it's inside a pre (handled by react-markdown), we probably don't want bg/px if it's block, 
                                        // but a simple style handles inline code.
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
                    <div className="p-4 rounded-2xl max-w-[90%] bg-black/40 text-gray-400 border border-white/10 animate-pulse">
                        <span className="inline-block w-2 h-2 bg-indigo-500 rounded-full mr-1 animate-bounce"></span>
                        <span className="inline-block w-2 h-2 bg-indigo-500 rounded-full mr-1 animate-bounce" style={{ animationDelay: '0.1s' }}></span>
                        <span className="inline-block w-2 h-2 bg-indigo-500 rounded-full animate-bounce" style={{ animationDelay: '0.2s' }}></span>
                    </div>
                )}
            </div>

            <form onSubmit={handleSend} className="flex gap-2 relative z-10 mt-auto">
                <input
                    type="text"
                    value={input}
                    onChange={(e) => setInput(e.target.value)}
                    placeholder="Ask Coach about your plan..."
                    className="flex-1 bg-black/40 border border-white/10 rounded-full px-5 py-3 text-white focus:outline-none focus:border-indigo-500/50 transition-colors placeholder:text-gray-600"
                    disabled={loading}
                />
                <button
                    type="submit"
                    disabled={loading || !input.trim()}
                    className="bg-indigo-500/20 hover:bg-indigo-500/40 text-indigo-300 font-medium py-3 px-8 rounded-full transition-all border border-indigo-500/30 disabled:opacity-50 disabled:cursor-not-allowed"
                >
                    Send
                </button>
            </form>
        </div>
    );
}
