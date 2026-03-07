import type { Metadata, Viewport } from "next";
import { Geist, Geist_Mono } from "next/font/google";
import "./globals.css";
import AnimationProvider from "./AnimationProvider";

const geistSans = Geist({
  variable: "--font-geist-sans",
  subsets: ["latin"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  viewportFit: "cover",
  themeColor: "#f87171",
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
        className={`${geistSans.variable} ${geistMono.variable} antialiased`}
      >
        {/* Ambient background glow orbs */}
        <div className="fixed inset-0 pointer-events-none overflow-hidden z-0" aria-hidden="true">
          <div className="ambient-glow-lg bg-red-500" style={{ top: '-10%', right: '-5%' }} />
          <div className="ambient-glow-lg bg-indigo-500" style={{ bottom: '-10%', left: '-5%' }} />
          <div className="ambient-glow bg-purple-500" style={{ top: '40%', left: '20%', width: '200px', height: '200px' }} />
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
