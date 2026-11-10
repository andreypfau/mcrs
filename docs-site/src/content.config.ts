import { defineCollection } from "astro:content";
import { z } from "astro/zod";
import { file } from "astro/loaders";
import { docsLoader } from "@astrojs/starlight/loaders";
import { docsSchema } from "@astrojs/starlight/schema";

const loc = z.object({ en: z.string(), ru: z.string() });

export const collections = {
  docs: defineCollection({
    loader: docsLoader(),
    schema: docsSchema({
      extend: z.object({
        kind: z.enum(["tutorial", "how-to", "reference", "explanation", "landing"]),
        audience: z.array(z.enum(["jvm", "contributor", "rust", "admin", "player", "observer"])),
        stability: z.enum(["stable", "changing", "wip"]),
        tracking: z.array(z.number()).default([]),
        sources: z.array(z.string()).default([]),
        verified: z.object({
          snapshot: z.string(),
          commit: z.string(),
          date: z.coerce.date(),
        }),
        translated_from: z.string().optional(),
      }),
    }),
  }),
  status: defineCollection({
    loader: file("src/data/status.yml"),
    schema: z.object({
      title: loc,
      works: loc,
      missing: loc,
      state: z.enum(["works", "partial", "wip"]),
      issues: z.array(z.number()).default([]),
      evidence: z.string(),
      verified: z.coerce.date(),
    }),
  }),
};
