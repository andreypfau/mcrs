import { defineCollection } from "astro:content";
import { z } from "astro/zod";
import { file } from "astro/loaders";
import { docsLoader, i18nLoader } from "@astrojs/starlight/loaders";
import { docsSchema, i18nSchema } from "@astrojs/starlight/schema";

const ui = z.object({
  "mcrs.state.works": z.string(), "mcrs.state.partial": z.string(), "mcrs.state.wip": z.string(),
  "mcrs.state.undecided": z.string(), "mcrs.state.declined": z.string(),
  "mcrs.banner.changing": z.string(), "mcrs.banner.wip": z.string(),
  "mcrs.verify.heading": z.string(), "mcrs.verify.verified": z.string(),
  "mcrs.verify.tracking": z.string(), "mcrs.verify.translatedFrom": z.string(),
  "mcrs.table.area": z.string(), "mcrs.table.state": z.string(),
  "mcrs.table.works": z.string(), "mcrs.table.missing": z.string(),
  "mcrs.demo.open": z.string(), "mcrs.sidebar.toggle": z.string(),
  "mcrs.freshness.title": z.string(), "mcrs.freshness.text": z.string(), "mcrs.freshness.dateLocale": z.string(),
  "mcrs.freshness.current": z.string(), "mcrs.freshness.behind.one": z.string(), "mcrs.freshness.behind.few": z.string(),
  "mcrs.freshness.behind.many": z.string(), "mcrs.freshness.behind.other": z.string(),
});

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
  i18n: defineCollection({ loader: i18nLoader(), schema: i18nSchema({ extend: ui }) }),
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
