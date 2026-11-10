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
        kind: z.enum(["tutorial", "how-to", "reference", "explanation", "landing"]).optional(),
        audience: z.array(z.enum(["jvm", "contributor", "rust", "admin", "player", "observer"])).optional(),
        stability: z.enum(["stable", "changing", "wip"]).optional(),
        tracking: z.array(z.number()).optional(),
        sources: z.array(z.string()).optional(),
        verified: z.object({
          snapshot: z.string(),
          commit: z.string(),
          date: z.coerce.date(),
        }).optional(),
        translated_from: z.string().optional(),
      }),
    }),
  }),
  issues: defineCollection({
    loader: file("src/data/issues.yml"),
    schema: z.object({ state: z.enum(["works", "partial", "wip", "undecided", "declined"]) }),
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
