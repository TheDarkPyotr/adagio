import type { MetadataRoute } from "next";
import { source } from "@/lib/source";

export const dynamic = "force-static";

const BASE_URL = "https://thedarkpyotr.github.io/adagio";

export default function sitemap(): MetadataRoute.Sitemap {
  return source.getPages().map((page) => ({
    url: `${BASE_URL}${page.url}`,
    lastModified: new Date(),
    changeFrequency: "weekly" as const,
    priority: page.url === "/docs" ? 1.0 : 0.8,
  }));
}
