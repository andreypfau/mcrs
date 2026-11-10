// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import mdx from "@astrojs/mdx";
import { REPO } from "./src/config.mjs";
import starlightLinksValidator from "starlight-links-validator";

export default defineConfig({
  site: "https://mc.rs",
  base: "/docs/",
  trailingSlash: "always",
  // Astro does not prepend base to redirect targets.
  redirects: { "/": "/docs/en/" },
  integrations: [
    starlight({
      title: "mcrs",
      description: "Minecraft Java Edition on Rust and Bevy ECS.",
      social: [
        { icon: "discord", label: "Discord", href: "https://discord.gg/ged3nMRzwG" },
        { icon: "github", label: "GitHub", href: REPO },
      ],
      defaultLocale: "en",
      locales: {
        en: { label: "English", lang: "en" },
        ru: { label: "Русский", lang: "ru" },
      },
      customCss: ["./src/styles/brand.css"],
      components: {
        PageTitle: "./src/components/overrides/PageTitle.astro",
        Footer: "./src/components/overrides/Footer.astro",
        PageSidebar: "./src/components/overrides/PageSidebar.astro",
      },
      plugins: [starlightLinksValidator({ errorOnFallbackPages: false })],
      editLink: { baseUrl: `${REPO}/edit/main/docs-site/` },
      lastUpdated: true,
      sidebar: [
        { label: "Docs home", translations: { ru: "Главная доков" }, link: "/" },
        { label: "Start", translations: { ru: "Начало" }, items: [{ autogenerate: { directory: "start" } }] },
        { label: "Plugin developers", translations: { ru: "Разработчикам плагинов" }, items: [{ autogenerate: { directory: "plugins" } }] },
        { label: "How it works", translations: { ru: "Как устроено" }, items: [{ autogenerate: { directory: "concepts" } }] },
        { label: "Crates", translations: { ru: "Крейты" }, items: [{ autogenerate: { directory: "crates" } }] },
        {
          label: "Server and client",
          translations: { ru: "Сервер и клиент" },
          items: [
            { label: "Server", translations: { ru: "Сервер" }, items: [{ autogenerate: { directory: "server" } }] },
            { label: "Client", translations: { ru: "Клиент" }, items: [{ autogenerate: { directory: "client" } }] },
          ],
        },
        { label: "Reference", translations: { ru: "Справочник" }, items: [{ autogenerate: { directory: "reference" } }] },
        { label: "Contributing", translations: { ru: "Участие" }, items: [{ autogenerate: { directory: "contributing" } }] },
        { label: "Project", translations: { ru: "О проекте" }, items: [{ autogenerate: { directory: "project" } }] },
      ],
    }),
    mdx(),
  ],
});
