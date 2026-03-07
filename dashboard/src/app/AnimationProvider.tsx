'use client';

import { useEffect } from 'react';

export default function AnimationProvider({ children }: { children: React.ReactNode }) {
    useEffect(() => {
        // Skip if reduced motion is preferred
        if (typeof window === 'undefined') return;
        const mq = window.matchMedia('(prefers-reduced-motion: reduce)');
        if (mq.matches) {
            // Make all section-reveal elements visible immediately
            document.querySelectorAll('.section-reveal').forEach(el => {
                el.classList.add('is-visible');
            });
            return;
        }

        const observer = new IntersectionObserver(
            (entries) => {
                entries.forEach(entry => {
                    if (entry.isIntersecting) {
                        entry.target.classList.add('is-visible');
                        observer.unobserve(entry.target);
                    }
                });
            },
            {
                threshold: 0.1,
                rootMargin: '0px 0px -40px 0px',
            }
        );

        // Observe on mount and on subsequent DOM mutations (for dynamic content)
        const observeAll = () => {
            document.querySelectorAll('.section-reveal:not(.is-visible)').forEach(el => {
                observer.observe(el);
            });
        };

        // Initial observation
        observeAll();

        // Watch for new elements (dynamic loading, navigation)
        const mutationObserver = new MutationObserver(() => {
            observeAll();
        });

        mutationObserver.observe(document.body, {
            childList: true,
            subtree: true,
        });

        return () => {
            observer.disconnect();
            mutationObserver.disconnect();
        };
    }, []);

    return <>{children}</>;
}
