import type { ReactNode } from "react";
import { RootProvider } from "fumadocs-ui/provider/next";
import "./globals.css";

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
    <html lang="en" suppressHydrationWarning data-theme="dark">
      <body>
        <RootProvider>{children}</RootProvider>
      </body>
    </html>
  );
}
