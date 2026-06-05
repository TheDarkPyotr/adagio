import type { ReactNode } from "react";
import localFont from "next/font/local";
import { RootProvider } from "fumadocs-ui/provider/next";
import "./globals.css";

const geist = localFont({
  src: [
    { path: "../public/fonts/Geist-Regular.woff2", weight: "400", style: "normal" },
    { path: "../public/fonts/Geist-Medium.woff2", weight: "500", style: "normal" },
  ],
  variable: "--font-sans",
  display: "swap",
});

const geistMono = localFont({
  src: [
    { path: "../public/fonts/GeistMono-Regular.woff2", weight: "400", style: "normal" },
    { path: "../public/fonts/GeistMono-Medium.woff2", weight: "500", style: "normal" },
  ],
  variable: "--font-mono",
  display: "swap",
});

export const metadata = {
  metadataBase: new URL("https://thedarkpyotr.github.io/adagio/"),
  title: {
    template: "%s | Adagio Docs",
    default: "Adagio Documentation",
  },
  description:
    "Documentation for Adagio — a conflict-aware, bandwidth-aware Nextcloud sync client for Linux.",
  openGraph: {
    siteName: "Adagio Documentation",
    images: [{ url: "/adagio_banner.png" }],
  },
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html
      lang="en"
      suppressHydrationWarning
      data-theme="dark"
      className={`${geist.variable} ${geistMono.variable}`}
    >
      <body>
        <RootProvider>{children}</RootProvider>
      </body>
    </html>
  );
}
