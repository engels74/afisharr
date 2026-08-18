---
type: "agent_requested"
description: "Bun + Svelte 5 + SvelteKit 2 + UnoCSS (presetWind4) + shadcn-svelte coding guidelines"
---

# Bun · Svelte 5 · SvelteKit 2 · UnoCSS (presetWind4) · shadcn-svelte — Authoritative Coding Reference

This stack is a fully compiler-driven, runes-first Svelte application served and tooled by Bun. Svelte 5 (runes) and SvelteKit 2 are the framework core; UnoCSS with `presetWind4` is the atomic CSS engine (Tailwind-v4-compatible, oklch + CSS-variable output); shadcn-svelte supplies copy-into-repo components built on Bits UI; and Bun is the runtime, package manager, test runner, and bundler. Optimize for: fine-grained reactivity via runes, server/client separation via SvelteKit's file conventions, and letting the compiler do the work (no virtual DOM, no runtime state wrappers).

The biggest way agents write wrong-but-plausible code here is by importing habits from adjacent ecosystems. From **Svelte 4**: `export let`, `$:`, `on:click`, slots, `createEventDispatcher`, stores-as-default-state. From **React**: `forwardRef`, `asChild`, `useState`, hook-style effects for derived data. From **Tailwind**: a `tailwind.config.js` file, `@tailwind` directives, `content` globbing. From **Node**: `dotenv`, `ts-node`, `jest`, `bcrypt`, hand-rolled `pg` pools. All of these are wrong on this stack. This document shows the one current idiomatic way for each, version-anchored so you know the floor.

## Stack snapshot

- **Research date:** August 8, 2026
- **Research basis:** current official docs, release notes, specifications, changelogs, and primary repositories.

| Component | Target | Notes |
|---|---|---|
| Bun | 1.3.x | Runtime + PM + test runner + bundler; text `bun.lock` (JSONC) is the default since v1.2 |
| Svelte | 5.4x+ | Runes stable since 5.0 (Oct 2024); attachments 5.29; async is **experimental** |
| SvelteKit | 2.5x+ | `$app/state` since 2.12; remote functions since 2.27 (**experimental**) |
| UnoCSS | 66.x | `presetWind4` is the Tailwind-v4-compatible preset |
| shadcn-svelte | current (runes-native) | Copy-into-repo; Bits UI primitives; Tailwind-v4 CSS-variable theming |
| bits-ui | latest | Svelte-native primitive library (the Radix equivalent) |

Critical stability note: **Svelte's `experimental.async` and SvelteKit's `experimental.remoteFunctions` are still experimental** as of the research date. The SvelteKit configuration docs state verbatim: "Experimental features. Here be dragons. These are not subject to semantic versioning, so breaking changes or removal can happen in any release," and the remote-functions flag is described as "not yet stable and may be changed or removed at any time." Treat these two features as opt-in for new/internal features only; do not build a production-critical API layer on them without pinning exact versions. Everything else in this document is stable.

## Bun as runtime, package manager, and toolchain

Bun replaces a pile of Node-era tools. Do **not** add `dotenv` (Bun reads `.env` automatically), `ts-node`/`tsx` (Bun runs `.ts` directly), `jest`/`ts-jest` (use `bun:test`), `bcrypt` (use `Bun.password`), or `nodemon` (use `bun --hot`).

### Package management and lockfile

Per Bun's official lockfile docs, "Since Bun v1.2, Bun uses a text-based lockfile called bun.lock (a JSONC format) by default" — commit it, and delete any legacy binary `bun.lockb` (to convert an existing one: `bun install --save-text-lockfile --frozen-lockfile --lockfile-only`). Use `bunx` (not `npx`) to execute package binaries.

```bash
bun install                 # install; writes/updates bun.lock
bun add bits-ui @lucide/svelte
bun add -d vitest @sveltejs/adapter-node
bun install --frozen-lockfile   # CI: fail if lockfile would change
bunx shadcn-svelte@latest add button
```

`bunfig.toml` — real config for this stack:

```toml
[install]
# The npm registry; use exact versions in production apps
registry = "https://registry.npmjs.org/"
exact = true

[install.scopes]
# private packages, if any
"@mycompany" = { url = "https://npm.mycompany.com/", token = "$NPM_TOKEN" }

[test]
# Register happy-dom (or other globals) before tests run
preload = ["./test-setup.ts"]
coverage = true
coverageReporter = ["text", "lcov"]
# Single number applies to lines/functions/statements; object sets each
coverageThreshold = { lines = 0.85, functions = 0.90, statements = 0.80 }
```

Monorepos use Bun **workspaces** (via `package.json` `workspaces`) and **catalogs** to pin shared dependency versions in one place. Use `--filter` to run scripts across packages.

### Running SvelteKit under Bun — the critical caveat

This is the single most misunderstood part of the stack. When you run `bun run dev`, Bun acts as the *package manager / script runner* but SvelteKit's Vite dev server still executes on **Node**. To force the **Bun runtime**, add the `--bun` flag:

```bash
bun --bun run dev     # Vite dev server runs on the Bun runtime
bun run dev           # Node runtime (Bun only launches the script)
```

For **production builds**, the adapter decides the runtime. `@sveltejs/adapter-node` is official and works when executed with `bun ./build/index.js`. `svelte-adapter-bun` is a **community** adapter that emits a standalone `Bun.serve()` server with native WebSocket support and Brotli/gzip precompression — use it only if you need Bun-native serving and can accept community maintenance. Be aware: with community Bun adapters, SvelteKit's CSRF origin check can break form actions unless `ORIGIN` (or `PROTOCOL_HEADER`/`HOST_HEADER` behind a trusted proxy) is set correctly.

| Adapter | Status | Use when |
|---|---|---|
| `@sveltejs/adapter-auto` | official | Zero-config deploys to supported platforms (Vercel/Netlify/Cloudflare) |
| `@sveltejs/adapter-node` | official | Self-hosted; run the output with `bun ./build/index.js` — safest default |
| `@sveltejs/adapter-static` | official | Fully prerendered SPA/SSG |
| `svelte-adapter-bun` | community | Standalone `Bun.serve()` server, native WS, precompression |

### Bun-native APIs

Use these instead of Node/third-party equivalents:

```ts
// Password hashing — Bun's docs state Bun.password.hash() "uses the Argon2id algorithm"
// by default (NOT bcrypt; no dependency needed). Output is PHC format: $argon2id$v=19$...
const hash = await Bun.password.hash(plaintext);
const ok   = await Bun.password.verify(plaintext, hash);  // algorithm auto-detected from hash

// Files
const file = Bun.file("./data.json");
const json = await file.json();
await Bun.write("./out.txt", "hello");

// Env — no dotenv; .env / .env.local read automatically
const port = Bun.env.PORT ?? "3000";

// Shell — cross-platform, auto-escaped interpolation
import { $ } from "bun";
const branch = await $`git rev-parse --abbrev-ref HEAD`.text();
```

`Bun.serve()` (with parameterized/catch-all routes, added in 1.3) is for standalone servers — inside SvelteKit you almost never call it directly; SvelteKit + adapter owns the server. Reach for it only in scripts or the community Bun adapter's output.

### Testing with Bun vs Vitest — be precise

`bun test` (Jest-compatible, imported from `bun:test`) is excellent and fast for **plain `.ts` logic** and **`.svelte.ts` rune/state modules**, but **it cannot compile `.svelte` component files** and SvelteKit's `$app/*` modules fail under it. So split your testing:

| Test target | Tool |
|---|---|
| Pure `.ts` utils, `.svelte.ts` state classes | `bun test` (native, fast) |
| `.svelte` component rendering | **Vitest + `vitest-browser-svelte`** (real browser via Playwright) |
| End-to-end | Playwright (`@playwright/test`) |

`bun test` with happy-dom for DOM-level logic tests (`test-setup.ts` preload registered in `bunfig.toml`):

```ts
// test-setup.ts
import { GlobalRegistrator } from "@happy-dom/global-registrator";
GlobalRegistrator.register();
```

```ts
// counter.svelte.test.ts  — testing a .svelte.ts state module with bun test
import { test, expect } from "bun:test";
import { createCounter } from "./counter.svelte.ts";

test("counter increments", () => {
  const c = createCounter(0);
  c.increment();
  expect(c.count).toBe(1);
});
```

`bun:test` also provides `mock()`, `spyOn()`, `mock.module()`, and snapshots (`toMatchSnapshot`, `toMatchInlineSnapshot`); update snapshots with `bun test -u`.

Component tests use Vitest browser mode (requires vitest 4+):

```ts
// vitest.config.ts
import { defineConfig } from "vitest/config";
import { sveltekit } from "@sveltejs/kit/vite";

export default defineConfig({
  plugins: [sveltekit()],
  test: {
    setupFiles: ["vitest-browser-svelte"],
    browser: { enabled: true, provider: "playwright", instances: [{ browser: "chromium" }] },
  },
});
```

```ts
// Button.svelte.test.ts
import { render } from "vitest-browser-svelte";
import { expect, test } from "vitest";
import Button from "./Button.svelte";

test("button click increments", async () => {
  const screen = render(Button, { initialCount: 1 });
  await screen.getByRole("button").click();
  await expect.element(screen.getByText("Count is 2")).toBeVisible();
});
```

Type-check with `svelte-check` (wire it to `bun run check`):

```jsonc
// package.json (scripts)
{
  "scripts": {
    "dev": "vite dev",
    "build": "vite build",
    "preview": "vite preview",
    "check": "svelte-kit sync && svelte-check --tsconfig ./tsconfig.json",
    "test:unit": "bun test",
    "test:browser": "vitest run",
    "lint": "biome ci .",
    "format": "biome format --write ."
  }
}
```

## Svelte 5 runes — reactivity done right

Runes are compile-time primitives (prefixed `$`), not function calls you import. They work in `.svelte` files and in `.svelte.ts`/`.svelte.js` modules. Writing Svelte 4 idioms (`export let`, `$:`, `on:click`, stores as default state) is the number-one failure mode — do not do it.

### State, derived, effects

```svelte
<script lang="ts">
  let count = $state(0);
  let doubled = $derived(count * 2);                 // pure, recomputed automatically
  let heavy = $derived.by(() => {                    // multi-line derivation
    let total = 0;
    for (let i = 0; i < count; i++) total += i;
    return total;
  });

  // $effect runs AFTER the DOM updates; use for side effects ONLY (not for deriving state)
  $effect(() => {
    document.title = `Count: ${count}`;
    return () => {/* cleanup on destroy / before re-run */};
  });
</script>

<button onclick={() => count++}>{count} → {doubled}</button>
```

Critical insight: **`$derived` is not `$effect`.** If you are computing a value from other state, use `$derived`/`$derived.by`. Using an `$effect` to write to another `$state` (effect-based syncing) creates extra renders and loops — it is the most common runes anti-pattern. Never reassign a `$derived` value manually; it is owned by the compiler.

Deep reactivity: `$state({...})` and `$state([...])` return a **deeply reactive Proxy** — mutating nested properties or `array.push()` triggers updates. Use `$state.raw(...)` when you want a value that only updates on reassignment (large immutable data, external instances). Use `$state.snapshot(x)` to get a plain, non-proxied clone (e.g. before passing to `structuredClone`, `JSON.stringify`, or a non-Svelte library).

```ts
let list = $state<{ id: number; done: boolean }[]>([]);
list.push({ id: 1, done: false });   // reactive
list[0].done = true;                  // reactive (deep proxy)

let config = $state.raw({ theme: "dark" });
config = { ...config, theme: "light" }; // only reassignment triggers updates

const plain = $state.snapshot(list);   // detached clone for serialization
```

`$effect.pre` runs before DOM updates; `$effect.root` creates a manually-disposed effect scope outside the component lifecycle; `untrack(fn)` reads state without creating a dependency. Use `tick()` to await DOM flush; `flushSync()` to force it synchronously (mainly in tests).

### Reactive state outside components — `.svelte.ts`

This replaces Svelte stores for most cases. Export a factory or a class; expose state via getters.

```ts
// counter.svelte.ts
export function createCounter(initial = 0) {
  let count = $state(initial);
  const doubled = $derived(count * 2);
  return {
    get count() { return count; },
    get doubled() { return doubled; },
    increment() { count++; },
    reset() { count = initial; },
  };
}
```

For reactive collections use the drop-in classes from `svelte/reactivity`: `SvelteMap`, `SvelteSet`, `SvelteDate`, `SvelteURL`, and `MediaQuery`. Plain `Map`/`Set`/`Date` are **not** reactive.

```ts
import { SvelteMap, MediaQuery } from "svelte/reactivity";
const cache = new SvelteMap<string, number>();
const prefersDark = new MediaQuery("(prefers-color-scheme: dark)");
// prefersDark.current is reactive
```

### Props, bindable, context

`$props()` replaces `export let`. Destructure with defaults and rename `class`. `$bindable()` marks a prop as two-way. `$props.id()` (5.20) generates a hydration-stable unique id.

```svelte
<script lang="ts">
  interface Props {
    title: string;
    count?: number;
    value?: string;          // bindable
    class?: string;
    children?: import("svelte").Snippet;
  }
  let {
    title,
    count = 0,
    value = $bindable(""),
    class: className,
    children,
  }: Props = $props();

  const uid = $props.id();   // stable across SSR/hydration
</script>

<label for={uid}>{title}</label>
<input id={uid} bind:value />
{@render children?.()}
```

Context uses `setContext`/`getContext` and pairs naturally with runes — put a `.svelte.ts` state object into context to share reactive state down a subtree.

### Snippets and `{@render}` replace slots

`createEventDispatcher` is gone; slots are gone. Pass markup as **snippet props** and call them with `{@render}`. Event handlers are plain props (`onclick`), and you communicate upward with callback props.

```svelte
<!-- List.svelte -->
<script lang="ts" generics="T">
  import type { Snippet } from "svelte";
  let { items, row, empty }: {
    items: T[];
    row: Snippet<[T]>;         // generics on snippets since 5.30
    empty?: Snippet;
  } = $props();
</script>

{#if items.length}
  <ul>{#each items as item (item)}<li>{@render row(item)}</li>{/each}</ul>
{:else}
  {@render empty?.()}
{/if}
```

```svelte
<!-- usage -->
<List items={users}>
  {#snippet row(user)}<span>{user.name}</span>{/snippet}
  {#snippet empty()}<p>No users</p>{/snippet}
</List>
```

Always key `{#each}` blocks with `(item.id)` when items can reorder or be removed — unkeyed each blocks reuse DOM by index and cause subtle state bugs.

### Attachments `{@attach}` replace actions (5.29)

Per the Svelte docs, "Attachments are available in Svelte 5.29 and newer." They are the modern replacement for `use:` actions: fully reactive (re-run when read state changes), inline-able, spreadable, and usable on components. Convert legacy library actions with `fromAction` (added in 5.32).

```svelte
<script lang="ts">
  import type { Attachment } from "svelte/attachments";
  import { fromAction } from "svelte/attachments";
  import { tooltip as tooltipAction } from "some-legacy-lib";

  function tooltip(content: string): Attachment {
    return (node) => {
      const t = createTooltip(node, content);   // runs on mount + when content changes
      return () => t.destroy();                  // cleanup
    };
  }
  let text = $state("Hi");
</script>

<button {@attach tooltip(text)}>hover</button>
<div {@attach fromAction(tooltipAction, () => text)}>legacy</div>
```

### The `class` attribute takes objects/arrays (5.16)

Since 5.16 the `class` attribute accepts objects/arrays and is merged with `clsx` under the hood. Prefer this over the legacy `class:` directive. Type incoming class props as `ClassValue`.

```svelte
<script lang="ts">
  import type { ClassValue } from "svelte/elements";
  let { class: className }: { class?: ClassValue } = $props();
  let active = $state(false);
</script>

<div class={["card", { active }, className]}>...</div>
```

### Error boundaries and mounting

`<svelte:boundary>` (5.3) catches errors in its subtree with `failed` and `pending` snippets. To mount components programmatically, use `mount`/`unmount`/`hydrate` from `svelte` — `new Component()` no longer works.

```svelte
<svelte:boundary>
  <RiskyComponent />
  {#snippet failed(error, reset)}
    <p>Something broke: {error.message}</p>
    <button onclick={reset}>Retry</button>
  {/snippet}
</svelte:boundary>
```

```ts
import { mount, unmount } from "svelte";
import App from "./App.svelte";
const app = mount(App, { target: document.getElementById("app")!, props: { name: "world" } });
// later: unmount(app);
```

Debug reactivity with `$inspect(value)` and `$inspect.trace()` (5.14) inside a function to log why it re-ran. These are stripped in production.

### Experimental async — do NOT use in production yet

Svelte's `await`-in-components (`experimental.async`) lets you `await` directly in the template/deriveds. Per Svelte's own release notes, experimental async SSR is "available in Svelte v5.39.3 and SvelteKit v2.43.0 or higher" and is opt-in via `experimental.async` — it remains **experimental** and is coupled to SvelteKit remote functions. Do not enable `compilerOptions.experimental.async` in production code paths; if you use it for prototyping, mark it clearly and pin exact versions.

## SvelteKit 2 — routing, loading, and the server boundary

SvelteKit's file conventions in `src/routes/` define the app. Learn what runs where; it prevents the most damaging mistakes (leaking secrets, shipping server code to the client).

### File conventions

| File | Runs | Purpose |
|---|---|---|
| `+page.svelte` | client + SSR | Page component |
| `+page.ts` | client + server | Universal `load` (runs both places) |
| `+page.server.ts` | server only | Server `load` + form `actions`; DB/secrets safe here |
| `+layout.svelte` / `+layout.server.ts` | — | Shared UI + data for a subtree |
| `+server.ts` | server only | API endpoints (`GET`/`POST`/…) returning `Response` |
| `+error.svelte` | client + SSR | Error UI boundary |
| `(group)` | — | Route group (organize without affecting URL) |
| `[param]`, `[...rest]`, `[[optional]]` | — | Dynamic / rest / optional params |

### load functions and typed data

Return serializable data from `load`. Universal `load` (`+page.ts`) can return non-serializable values (class instances, components) and runs on both sides; server `load` (`+page.server.ts`) runs only on the server and its return is devalue-serialized to the client. Type everything with the generated `./$types`.

```ts
// src/routes/blog/[slug]/+page.server.ts
import { error } from "@sveltejs/kit";
import type { PageServerLoad } from "./$types";
import { db } from "$lib/server/db";

export const load: PageServerLoad = async ({ params, locals, setHeaders }) => {
  const post = db.query("SELECT * FROM post WHERE slug = ?").get(params.slug);
  if (!post) error(404, "Not found");            // SvelteKit 2: no `throw` needed
  setHeaders({ "cache-control": "max-age=60" });
  return { post, user: locals.user };
};
```

```svelte
<!-- +page.svelte -->
<script lang="ts">
  import type { PageProps } from "./$types";
  let { data }: PageProps = $props();
</script>
<h1>{data.post.title}</h1>
```

Critical insight — **SvelteKit 2 throw semantics**: `error()`, `redirect()` are called directly (not thrown). Guard genuine errors with `isHttpError`/`isRedirect` from `@sveltejs/kit` when catching.

Stream slow data by returning a **promise** from a server `load` (top-level keys resolve first, nested promises stream in):

```ts
export const load: PageServerLoad = async () => ({
  fast: await getCriticalData(),
  slow: getSlowData(),          // a promise — streams to the client
});
```

```svelte
{#await data.slow}<Spinner />{:then value}{value}{/await}
```

### Form actions and progressive enhancement

Form actions in `+page.server.ts` are the default way to mutate server state. They work without JS and are enhanced with `use:enhance`. Return validation failures with `fail()`.

```ts
// +page.server.ts
import { fail, redirect } from "@sveltejs/kit";
import type { Actions } from "./$types";

export const actions: Actions = {
  login: async ({ request, cookies, locals }) => {
    const data = await request.formData();
    const email = String(data.get("email"));
    if (!email) return fail(400, { email, missing: true });
    const session = await createSession(email);
    cookies.set("session", session.id, { path: "/", httpOnly: true, secure: true, sameSite: "lax" });
    redirect(303, "/dashboard");
  },
};
```

```svelte
<script lang="ts">
  import { enhance } from "$app/forms";
  import type { PageProps } from "./$types";
  let { form }: PageProps = $props();   // action result
</script>

<form method="POST" action="?/login" use:enhance>
  <input name="email" value={form?.email ?? ""} />
  {#if form?.missing}<span>Email required</span>{/if}
  <button>Log in</button>
</form>
```

### hooks and locals — auth belongs here

`src/hooks.server.ts` `handle` runs on every request; populate `event.locals` for auth. `handleFetch` rewrites server-side `fetch`; `handleError` shapes error reporting. The `transport` hook (`hooks.ts`, 2.11) lets you serialize/deserialize custom types (e.g. `Decimal`, `Temporal`) across the server/client boundary.

```ts
// src/hooks.server.ts
import type { Handle } from "@sveltejs/kit";
import { redirect } from "@sveltejs/kit";

export const handle: Handle = async ({ event, resolve }) => {
  const sessionId = event.cookies.get("session");
  event.locals.user = sessionId ? await getUser(sessionId) : null;

  if (event.url.pathname.startsWith("/dashboard") && !event.locals.user) {
    redirect(303, "/login");
  }
  return resolve(event);
};
```

```ts
// src/app.d.ts — type your locals
declare global {
  namespace App {
    interface Locals { user: { id: string; email: string } | null; }
  }
}
export {};
```

### State access — `$app/state`, not `$app/stores` (2.12)

`$app/state` exposes `page`, `navigating`, `updated` as fine-grained runes-based objects (each backed by `$state.raw` under the hood). Use it — `$app/stores` (the `$page` store form) is deprecated and slated for removal in SvelteKit 3.

```svelte
<script lang="ts">
  import { page, navigating } from "$app/state";   // NOT $app/stores
</script>
<nav class:active={page.url.pathname === "/"}>...</nav>
{#if navigating.to}<div class="loading-bar" />{/if}
```

Navigation helpers live in `$app/navigation`: `goto`, `invalidate`, `invalidateAll`, `preloadData`, `pushState`/`replaceState` (shallow routing). Pair `depends("app:data")` in `load` with `invalidate("app:data")` to re-run a specific load. Shallow routing with `pushState` + `page.state` powers modals-as-history-entries.

### Environment variables — the four modules

| Module | Values | Where |
|---|---|---|
| `$env/static/private` | build-time secrets | server only |
| `$env/static/public` | build-time, `PUBLIC_`-prefixed | client + server |
| `$env/dynamic/private` | runtime secrets | server only |
| `$env/dynamic/public` | runtime, `PUBLIC_`-prefixed | client + server |

```ts
import { DATABASE_URL } from "$env/static/private";   // never reaches the client
import { PUBLIC_API_BASE } from "$env/static/public";
```

Server-only code should live under `$lib/server/` — SvelteKit hard-errors if a `$lib/server` module is imported into client code, which is your safety net for DB clients and secrets.

### Remote functions — experimental, opt-in

Remote functions (`query`, `form`, `command`, `prerender` from `$app/server`) are documented as "Available since 2.27" and let you call type-safe server functions from anywhere, defined in `.remote.ts` files. They are **experimental** (require `kit.experimental.remoteFunctions` + `compilerOptions.experimental.async`), every function becomes a public HTTP endpoint (so validate inputs with a Standard Schema library like Zod/Valibot), and the API has continued to take breaking changes across minor versions. Use them for internal data flows on new features; keep `+server.ts` for public/webhook APIs. If you adopt them, pin your exact SvelteKit version.

```ts
// data.remote.ts  (experimental)
import { query } from "$app/server";
import * as v from "valibot";
import { db } from "$lib/server/db";

export const getPost = query(v.string(), async (slug) => {
  return db.query("SELECT * FROM post WHERE slug = ?").get(slug);
});
```

### svelte.config.js and vite.config.ts

```js
// svelte.config.js
import adapter from "@sveltejs/adapter-node";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

export default {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter(),
    alias: { $lib: "src/lib" },
    // CSP example
    csp: { directives: { "script-src": ["self"] } },
  },
};
```

```ts
// vite.config.ts
import { sveltekit } from "@sveltejs/kit/vite";
import UnoCSS from "unocss/vite";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [UnoCSS(), sveltekit()],   // UnoCSS BEFORE sveltekit
});
```

### Server-side database access with Bun

Inside `+page.server.ts` / `+server.ts`, use Bun's native drivers. For SQLite, `bun:sqlite`:

```ts
// src/lib/server/db.ts
import { Database } from "bun:sqlite";
export const db = new Database("app.sqlite", { strict: true });
db.run("PRAGMA journal_mode = WAL;");
```

```ts
// usage in a load
const posts = db.query("SELECT * FROM post ORDER BY created_at DESC LIMIT ?").all(20);
const post  = db.query("SELECT * FROM post WHERE id = $id").get({ id });
```

For Postgres, use `Bun.sql` (tagged templates auto-parameterize — injection-safe; no `pg` needed):

```ts
import { sql } from "bun";
const users = await sql`SELECT * FROM users WHERE active = ${true} LIMIT ${10}`;
```

## UnoCSS with presetWind4

`presetWind4` is the **Tailwind-v4-compatible** preset — it is the current target. Do not use `presetUno` or `presetWind3`; those are the legacy/superseded preset names. There is **no `tailwind.config.js`** and **no `@tailwind` directive** on this stack — that is a Tailwind habit that does not apply. presetWind4 emits oklch colors and uses CSS variables + `@property` in dedicated `theme`/`properties` layers, and it **includes its own reset** (no separate `@unocss/reset` install needed).

### uno.config.ts

```ts
// uno.config.ts
import { defineConfig, presetWind4, presetIcons, presetWebFonts, transformerVariantGroup } from "unocss";

export default defineConfig({
  presets: [
    presetWind4({
      preflights: { reset: true },   // built-in Tailwind-v4-aligned reset
    }),
    presetIcons({ scale: 1.2, extraProperties: { display: "inline-block", "vertical-align": "middle" } }),
    presetWebFonts({ themeKey: "font", provider: "google", fonts: { sans: "Inter:400,500,600,700" } }),
  ],
  transformers: [transformerVariantGroup()],
  shortcuts: {
    "btn": "px-4 py-2 rounded bg-primary text-white hover:bg-primary/90 disabled:opacity-50",
  },
  theme: {
    colors: { brand: { DEFAULT: "#4f46e5", muted: "#6366f1" } },
  },
  safelist: ["i-lucide-loader-2"],   // classes generated dynamically
});
```

Key gotchas anchored to presetWind4:
- **`transformerDirectives` (`@apply`, `@screen`) has known issues with presetWind4** — the docs warn to use it with caution, and `@screen` breaks because breakpoints moved out of config. Prefer `shortcuts` over `@apply` on this stack.
- presetWind4 uses the oklch color model and is **incompatible with `presetLegacyCompat`** and `presetRemToPx` — don't combine them.
- Fonts: with presetWind4, `presetWebFonts` uses `themeKey: 'font'` (the old `fontFamily` theme key is unsupported).

### SvelteKit integration — global vs svelte-scoped

Two integration modes. Choose **global mode** (`unocss/vite`) for apps — it is simpler and required for shadcn-svelte compatibility. Use **svelte-scoped** (`@unocss/svelte-scoped/vite`) only for component *libraries* where you must ship self-contained per-component styles.

Global mode (recommended default):

```ts
// vite.config.ts
import { sveltekit } from "@sveltejs/kit/vite";
import UnoCSS from "unocss/vite";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [UnoCSS(), sveltekit()],
});
```

```ts
// src/routes/+layout.svelte  (or app entry)
import "virtual:uno.css";
```

Svelte-scoped mode (library use) requires the `%unocss-svelte-scoped.global%` placeholder in `app.html` and a `transformPageChunk` hook in `hooks.server.ts` — reach for it only when packaging components.

### Making shadcn-svelte (Tailwind-first) work on UnoCSS

shadcn-svelte components are authored with Tailwind class names and expect Tailwind-v4-style CSS variables (`--background`, `--primary`, oklch values). presetWind4 targets Tailwind v4, so most utilities map directly, but be deliberate:

- **Theme tokens:** define shadcn's design tokens as CSS variables in your global stylesheet (`:root` and `.dark`), exactly as shadcn's `init` generates them. presetWind4's variable-based output coexists with these.
- **`cn()` still uses `tailwind-merge`:** shadcn-svelte's `cn()` helper merges with `tailwind-merge` + `clsx`. This works because presetWind4 emits Tailwind-compatible class names; keep `cn()` as shipped.
- **Animations:** shadcn/Tailwind v4 projects use `tw-animate-css` (the successor to the discontinued `tailwindcss-animate`). On UnoCSS, provide the `accordion-down`/`accordion-up` and enter/exit keyframes via your `uno.config.ts` `theme.animation`/`preflights` or a small CSS shim, since there is no `@plugin` mechanism.
- **Dark mode:** use the **class strategy** (`.dark` on `<html>`), driven by `mode-watcher`. presetWind4's `dark:` variant keys off that class.
- **No `@theme`/`@plugin` directives:** those are Tailwind-v4 CSS-file features. On UnoCSS, put tokens in `:root`/`.dark` CSS and configure the rest in `uno.config.ts`.

## shadcn-svelte — components you own

shadcn-svelte is **not a component library** — the CLI copies component source into your repo (default `$lib/components/ui`), and you edit those files directly. It is runes-native (Svelte 5) and Tailwind-v4-ready. Components wrap **Bits UI** primitives (the Svelte-native replacement for Radix). There is no `React.forwardRef` and no `asChild` — those are React idioms. Composition is done with the `child` snippet and Svelte snippets.

### Setup

```bash
bunx shadcn-svelte@latest init      # writes components.json, utils.ts (cn), CSS variables
bunx shadcn-svelte@latest add button card dialog
```

`components.json` (current shape — note `registry` at root and `hooks`/`ui`/`lib` aliases):

```jsonc
{
  "$schema": "https://shadcn-svelte.com/schema.json",
  "style": "default",
  "tailwind": { "css": "src/app.css", "baseColor": "slate" },
  "aliases": {
    "components": "$lib/components",
    "utils": "$lib/utils",
    "ui": "$lib/components/ui",
    "hooks": "$lib/hooks",
    "lib": "$lib"
  },
  "typescript": true,
  "registry": "https://shadcn-svelte.com/registry"
}
```

`utils.ts` — the `cn()` helper plus the type helpers shadcn-svelte now bundles (previously imported from bits-ui):

```ts
// src/lib/utils.ts
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

// biome-ignore lint/suspicious/noExplicitAny: structural probe, the shape is irrelevant
export type WithoutChild<T> = T extends { child?: any } ? Omit<T, "child"> : T;
// biome-ignore lint/suspicious/noExplicitAny: structural probe, the shape is irrelevant
export type WithoutChildren<T> = T extends { children?: any } ? Omit<T, "children"> : T;
export type WithoutChildrenOrChild<T> = WithoutChildren<WithoutChild<T>>;
export type WithElementRef<T, U extends HTMLElement = HTMLElement> = T & { ref?: U | null };
```

### Component authoring idiom

The house style: destructure `ref = $bindable(null)`, rename `class`, spread `...restProps`, forward `bind:ref`, add a `data-slot` attribute, and render `children` with `{@render}`. Styling variants use **`tailwind-variants`** (`tv`) in current versions (not `class-variance-authority`).

```svelte
<!-- $lib/components/ui/button/button.svelte -->
<script lang="ts" module>
  import { tv, type VariantProps } from "tailwind-variants";
  export const buttonVariants = tv({
    base: "inline-flex items-center justify-center rounded-md text-sm font-medium transition-colors disabled:opacity-50 disabled:pointer-events-none",
    variants: {
      variant: {
        default: "bg-primary text-primary-foreground hover:bg-primary/90",
        outline: "border border-input bg-background hover:bg-accent",
        ghost: "hover:bg-accent hover:text-accent-foreground",
      },
      size: { default: "h-9 px-4 py-2", sm: "h-8 px-3", icon: "size-9" },
    },
    defaultVariants: { variant: "default", size: "default" },
  });
  export type ButtonVariant = VariantProps<typeof buttonVariants>["variant"];
  export type ButtonSize = VariantProps<typeof buttonVariants>["size"];
</script>

<script lang="ts">
  import type { HTMLButtonAttributes } from "svelte/elements";
  import { cn, type WithElementRef } from "$lib/utils.js";

  let {
    ref = $bindable(null),
    class: className,
    variant = "default",
    size = "default",
    children,
    ...restProps
  }: WithElementRef<HTMLButtonAttributes> & { variant?: ButtonVariant; size?: ButtonSize } = $props();
</script>

<button
  bind:this={ref}
  data-slot="button"
  class={cn(buttonVariants({ variant, size }), className)}
  {...restProps}
>
  {@render children?.()}
</button>
```

Wrapping a Bits UI primitive (note `WithoutChild`, `bind:ref`, `data-slot`):

```svelte
<!-- $lib/components/ui/accordion/accordion-content.svelte -->
<script lang="ts">
  import { Accordion as AccordionPrimitive } from "bits-ui";
  import { cn, type WithoutChild } from "$lib/utils.js";
  let { ref = $bindable(null), class: className, children, ...restProps }:
    WithoutChild<AccordionPrimitive.ContentProps> = $props();
</script>

<AccordionPrimitive.Content
  bind:ref
  data-slot="accordion-content"
  class="overflow-hidden text-sm data-[state=open]:animate-accordion-down data-[state=closed]:animate-accordion-up"
  {...restProps}
>
  <div class={cn("pb-4 pt-0", className)}>{@render children?.()}</div>
</AccordionPrimitive.Content>
```

### `child` snippet instead of `asChild`

To render a Bits UI trigger as your own element/component, use the `child` snippet (this is the Svelte replacement for React's `asChild`):

```svelte
<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { buttonVariants } from "$lib/components/ui/button/index.js";
</script>

<Dialog.Root>
  <Dialog.Trigger>
    {#snippet child({ props })}
      <a href="/settings" class={buttonVariants({ variant: "outline" })} {...props}>Open</a>
    {/snippet}
  </Dialog.Trigger>
  <Dialog.Content>...</Dialog.Content>
</Dialog.Root>
```

### Namespace imports and icons

Compound components are imported as namespaces; icons come from **`@lucide/svelte`** (the scoped package — match it; the unscoped `lucide-svelte` is the older name and mixing them ships two icon libs).

```svelte
<script lang="ts">
  import * as Card from "$lib/components/ui/card/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import Settings from "@lucide/svelte/icons/settings";
</script>

<Card.Root>
  <Card.Header><Card.Title>Title</Card.Title></Card.Header>
  <Card.Content>...</Card.Content>
  <Card.Footer><Button><Settings class="mr-2 size-4" />Save</Button></Card.Footer>
</Card.Root>
```

### Dark mode with mode-watcher

Use `mode-watcher` — it sets the theme **before paint** (avoiding the light→dark flash you get if you set the class in `onMount`). Put `<ModeWatcher />` in the root layout.

```svelte
<!-- src/routes/+layout.svelte -->
<script lang="ts">
  import "../app.css";
  import { ModeWatcher } from "mode-watcher";
  let { children } = $props();
</script>

<ModeWatcher />
{@render children?.()}
```

```svelte
<!-- theme toggle -->
<script lang="ts">
  import Sun from "@lucide/svelte/icons/sun";
  import Moon from "@lucide/svelte/icons/moon";
  import { toggleMode } from "mode-watcher";
  import { Button } from "$lib/components/ui/button/index.js";
</script>

<Button variant="outline" size="icon" onclick={toggleMode}>
  <Sun class="size-5 dark:hidden" />
  <Moon class="hidden size-5 dark:block" />
</Button>
```

### Forms: superforms + formsnap + zod

The current form stack is **sveltekit-superforms** + **formsnap** + a Standard Schema validator (Zod or Valibot). This gives typed, progressively-enhanced forms wired to SvelteKit actions.

```ts
// +page.server.ts
import { superValidate } from "sveltekit-superforms";
import { zod } from "sveltekit-superforms/adapters";
import { z } from "zod";
import { fail } from "@sveltejs/kit";

const schema = z.object({ email: z.string().email(), name: z.string().min(2) });

export const load = async () => ({ form: await superValidate(zod(schema)) });
export const actions = {
  default: async ({ request }) => {
    const form = await superValidate(request, zod(schema));
    if (!form.valid) return fail(400, { form });
    // persist...
    return { form };
  },
};
```

### Theming with CSS variables

shadcn's tokens are CSS variables in oklch (Tailwind-v4 era) defined in `app.css`:

```css
:root {
  --background: oklch(1 0 0);
  --foreground: oklch(0.145 0 0);
  --primary: oklch(0.205 0 0);
  --primary-foreground: oklch(0.985 0 0);
  --radius: 0.625rem;
}
.dark {
  --background: oklch(0.145 0 0);
  --foreground: oklch(0.985 0 0);
  --primary: oklch(0.985 0 0);
  --primary-foreground: oklch(0.205 0 0);
}
```

## Tooling: Biome, TypeScript

**Biome is the only linter and formatter.** One Rust binary, one config file, one pass over the tree. Scaffold with `bunx sv create`, then add Biome — take **no** linter or formatter add-ons from `sv add`:

```sh
bun add -D --exact @biomejs/biome
bunx biome init
```

Three commands cover the whole workflow:

| Command | What it does |
|---|---|
| `biome ci .` | Checks format, lint, and assist actions. Writes nothing. Use in pre-commit and CI. |
| `biome check --write .` | Same checks, applies every safe fix. Use while developing. |
| `biome format --write .` | Formats only. |

Suppress a rule inline with `// biome-ignore lint/<group>/<rule>: <reason>`. The reason is mandatory; Biome rejects a bare ignore.

Biome parses `.svelte` files natively since v2.3, and v2.4 added the Svelte control-flow syntax (`{#if}`, `{#each}`). Two caveats that decide how you configure it:

1. Without `html.experimentalFullSupportEnabled`, Biome touches only the `<script>` and `<style>` blocks and leaves the template markup alone. Turn it on to format the markup too, and accept that the support is still experimental.
2. Biome does not type-check. `svelte-check` stays in the toolchain and stays in the pre-commit gate.

```jsonc
// biome.json
{
  "$schema": "https://biomejs.dev/schemas/2.5.7/schema.json",
  "vcs": { "enabled": true, "clientKind": "git", "useIgnoreFile": true },
  "files": { "includes": ["**", "!build", "!.svelte-kit", "!dist"] },
  "formatter": { "enabled": true, "indentStyle": "tab" },
  "linter": { "enabled": true, "rules": { "preset": "recommended" } },
  "assist": { "actions": { "source": { "organizeImports": "on" } } },
  "javascript": {
    "formatter": { "quoteStyle": "single", "trailingCommas": "all" }
  },
  // Formats the Svelte template markup, not just <script> and <style>.
  // Experimental — drop this line if the markup output surprises you.
  "html": { "experimentalFullSupportEnabled": true }
}
```

Pin the version exactly (`--exact`). Biome ships formatter changes in minor releases, so a floating range makes the whole tree reformat on an unrelated `bun install`.

```jsonc
// tsconfig.json
{
  "extends": "./.svelte-kit/tsconfig.json",
  "compilerOptions": {
    "allowJs": true,
    "checkJs": true,
    "esModuleInterop": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "skipLibCheck": true,
    "sourceMap": true,
    "strict": true,
    "moduleResolution": "bundler"
  }
}
```

## Anti-patterns to avoid

| Wrong (don't) | Right (do) | Why |
|---|---|---|
| `export let count;` | `let { count } = $props();` | Svelte 4 props are gone in runes mode |
| `$: doubled = count * 2;` | `let doubled = $derived(count * 2);` | `$:` is Svelte 4; not reactive in runes |
| `on:click={fn}` | `onclick={fn}` | Event handlers are plain props now |
| `$effect(() => { doubled = count * 2; })` | `let doubled = $derived(...)` | Effect-based syncing loops; derive instead |
| Mutating a `$derived` value | Derive from source; mutate the source | Deriveds are compiler-owned |
| `createEventDispatcher()` | Callback props (`onsave={...}`) | Removed in Svelte 5 |
| `<slot />` | Snippet props + `{@render children?.()}` | Slots replaced by snippets |
| `new Component({ target })` | `mount(Component, { target })` | Class instantiation removed |
| Plain `Map`/`Set`/`Date` for reactive data | `SvelteMap`/`SvelteSet`/`SvelteDate` | Native ones aren't reactive |
| `import { page } from "$app/stores"` (`$page`) | `import { page } from "$app/state"` | Stores deprecated since 2.12 |
| `throw error(404)` / `throw redirect(303, …)` | `error(404)` / `redirect(303, …)` | SvelteKit 2 no longer needs `throw` |
| Secrets in `+page.ts` or `$lib` | `$env/static/private` in `+page.server.ts` / `$lib/server` | Universal/client code ships to browser |
| `import "dotenv/config"` | `Bun.env` / `$env/*` | Bun reads `.env` automatically |
| `bcrypt` package | `Bun.password.hash` (argon2id) | Native, no dependency |
| `presetUno` / `presetWind3` | `presetWind4` | Legacy/superseded preset names |
| `tailwind.config.js` + `@tailwind` | `uno.config.ts` | Tailwind config doesn't apply to UnoCSS |
| `use:action` for new element behavior | `{@attach ...}` | Actions superseded by attachments (5.29) |
| `asChild` prop (React) | `child` snippet | Bits UI uses the `child` snippet |
| `React.forwardRef` | `ref = $bindable(null)` + `bind:this` | React idiom; not Svelte |
| `class-variance-authority` | `tailwind-variants` (`tv`) | Current shadcn-svelte variant tool |
| `lucide-svelte` mixed with scoped | `@lucide/svelte` | Match the scoped package to avoid dupes |
| `jest` / `ts-jest` | `bun test` + `vitest-browser-svelte` | Bun native + browser-mode components |
| `bun test` on `.svelte` components | Vitest browser mode | `bun test` can't compile `.svelte` |
| enabling `experimental.async` in prod | Keep it opt-in for prototypes | Not semver-protected; may break |
