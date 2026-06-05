import type { ReactNode } from "react";
import Image from "next/image";
import { DocsLayout } from "fumadocs-ui/layouts/docs";
import { source } from "@/lib/source";

export default function Layout({ children }: { children: ReactNode }) {
  return (
    <DocsLayout
      tree={source.pageTree}
      githubUrl="https://github.com/TheDarkPyotr/adagio"
      nav={{
        title: (
          <span style={{ display: "flex", alignItems: "center", gap: "8px" }}>
            <Image
              src="/adagio_banner.png"
              alt="Adagio"
              width={24}
              height={24}
            />
            Adagio
          </span>
        ),
      }}
      themeSwitch={{ enabled: false }}
    >
      {children}
    </DocsLayout>
  );
}
