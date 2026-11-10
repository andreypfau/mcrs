import { getEntry } from "astro:content";

export interface PageMeta {
  kind: string;
  audience: string[];
  stability: "stable" | "changing" | "wip";
  tracking: number[];
  sources: string[];
  verified: { snapshot: string; commit: string; date: Date };
  translatedFrom?: string;
}

const inherited = ["kind", "audience", "stability", "tracking", "sources", "verified"] as const;

export async function pageMeta(entry: { id: string; data: Record<string, unknown> }): Promise<PageMeta | null> {
  const isRu = entry.id === "ru" || entry.id.startsWith("ru/");
  if (!isRu && !entry.id.startsWith("en/") && entry.id !== "en") return null;
  let src = entry.data;
  let translatedFrom: string | undefined;
  if (isRu) {
    const own = inherited.filter((k) => entry.data[k] !== undefined);
    if (own.length) throw new Error(`${entry.id}: Russian page sets inherited fields: ${own.join(", ")}`);
    translatedFrom = entry.data.translated_from as string | undefined;
    if (!translatedFrom) throw new Error(`${entry.id}: Russian page has no translated_from`);
    const pairId = entry.id === "ru" ? "en" : entry.id.replace(/^ru\//, "en/");
    const pair = await getEntry("docs", pairId);
    if (!pair) throw new Error(`${entry.id}: no English pair ${pairId}`);
    src = pair.data;
  }
  for (const k of ["kind", "audience", "stability", "verified"]) {
    if (src[k] === undefined) throw new Error(`${isRu ? entry.id + " (via English pair)" : entry.id}: missing ${k}`);
  }
  const stability = src.stability as PageMeta["stability"];
  const tracking = (src.tracking as number[] | undefined) ?? [];
  if (stability !== "stable" && tracking.length === 0) {
    throw new Error(`${entry.id}: stability "${stability}" requires non-empty tracking`);
  }
  return {
    kind: src.kind as string,
    audience: src.audience as string[],
    stability,
    tracking,
    sources: (src.sources as string[] | undefined) ?? [],
    verified: src.verified as PageMeta["verified"],
    translatedFrom,
  };
}
