import type { Metadata, Viewport } from "next";
import { Inter } from "next/font/google";
import "./globals.css";
import AnimationProvider from "./AnimationProvider";

const inter = Inter({
  variable: "--font-inter",
  subsets: ["latin"],
  display: "swap",
});

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  viewportFit: "cover",
  themeColor: "#ff6b6b",
};

export const metadata: Metadata = {
  title: "Fitness Journal",
  description: "Your personal fitness and workout journal",
  manifest: "/manifest.json",
  appleWebApp: {
    capable: true,
    statusBarStyle: "black-translucent",
    title: "Fitness Journal",
  },
  icons: {
    apple: "/icons/apple-touch-icon.png",
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body
        className={`${inter.variable} antialiased`}
        style={{ fontFamily: "var(--font-inter), var(--font-sans)" }}
      >
        {/* Subtle ambient background */}
        <div className="fixed inset-0 pointer-events-none overflow-hidden z-0" aria-hidden="true">
          <div className="ambient-glow-lg bg-red-500" style={{ top: '-8%', right: '-3%' }} />
          <div className="ambient-glow-lg bg-indigo-500" style={{ bottom: '-8%', left: '-3%' }} />
        </div>

        <AnimationProvider>
          <div className="relative z-10">
            {children}
          </div>
        </AnimationProvider>
      </body>
    </html>
  );
}
