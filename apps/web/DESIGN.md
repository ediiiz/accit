# DESIGN.md

## Visual language

Theme tokens live in `src/app.css`. The presets (`graphite`, `zinc`, `stone`, `slate`) are listed in `#lib/theme-presets.ts` and checked by `theme-contrast.svelte.test.ts`. Replace them with the product's palette when the starter becomes a product.

### Shell

```text
┌────────────── --sidebar canvas ──────────────────────────────┐
│ ▣ Brand            ┌──────── --background panel ──────────┐  │
│ [+ New project]    │ ☰ │ Projects › Detail      ● Synced  │  │ ← 48px sticky top bar, blurred
│ WORKSPACE          ├──────────────────────────────────────┤  │
│ ⌂ Home             │                                      │  │
│ ▦ Projects         │   page content (only this scrolls)   │  │
│ RECENT PROJECTS    │                                      │  │
│ ◔ Alpha     2/5    │                                      │  │
│ 🌐 English   ☾     │                                      │  │
│ (avatar) Name      └──────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

- `Sidebar.Root variant="inset" collapsible="icon"`. Collapsed, it becomes an icon rail with tooltips. Groups that make no sense as icons (recent projects, locale) hide with `group-data-[collapsible=icon]:hidden`.
- The page is a floating panel: `Sidebar.Inset` with `ring-1 ring-border` and a soft shadow, `m-2` on desktop. On mobile the panel goes edge to edge and `app-bottom-nav.svelte` replaces the sidebar: a floating, icon-only bar with the workspace destinations (`navItemsFor` in `#lib/nav.ts`) plus a menu button that opens the sidebar as a sheet for everything else.
- Sidebar groups follow the permission layers: **Workspace** (Home, Projects) for everyone; **Management** (Members, Teams, Roles of the active organization) only for members with a management permission (`canManageOrganization` in `#lib/org-roles.ts`); **Admin** (Users, Organizations) only for app admins. Settings live in the user menu.
- `app-topbar.svelte` (desktop only; mobile drops it to give the page the full height) owns the sidebar trigger, a URL-derived breadcrumb and the live `connection-status.svelte` chip (real `/api/live` WebSocket state, not a decorative dot).

### Signature elements

Use these to create the "impressive" moments. Each has one job.

| Element                                                      | Where                             | Job                                                                  |
| ------------------------------------------------------------ | --------------------------------- | -------------------------------------------------------------------- |
| `bg-grid` + `bg-spotlight` utilities                         | Home hero, auth layout            | Atmosphere: a faint grid lit by an indigo cone from the top edge.    |
| `text-gradient` utility                                      | Hero and login headings only      | Top-lit display type. Keeps `color` set as a fallback.               |
| `hairline-beam` utility                                      | Bottom edge of the hero           | A 1px light beam that separates the hero from working UI.            |
| `brand-mark.svelte`                                          | Sidebar header, login (`glow`)    | The logo tile. Replace the glyph when the starter becomes a product. |
| `progress-ring.svelte`                                       | Every project row, project header | Linear-style status glyph: empty ring, arc, filled check.            |
| `animated-number.svelte`                                     | Home totals                       | Counts up on mount, eases between live updates.                      |
| Indigo button glow (`shadow-[0_8px_30px_-8px_var(--brand)]`) | One primary CTA per screen        | Marks the single most important action.                              |

Decoration stops at the hero. Routine product screens (lists, forms, detail pages) get no grid, no spotlight and no gradient text.

### Surfaces

- **Page headings are layout, not cards.** Use `page-header.svelte`: eyebrow, title, description, optional `leading` visual and `actions`, closed by a bottom hairline.
- **Lists are one surface with dividers**, not a stack of cards: `Item.Group class="bg-card gap-0 divide-y overflow-hidden rounded-xl border"` with `Item.Root class="rounded-none border-0"` rows. Rows get `hover:bg-muted/60`; linked rows reveal a caret on hover.
- **Stats are a hairline-divided strip** (`<dl>` in a grid with `border-l`/`border-t`), never a mosaic of stat cards.
- **Cards are for interactions**: a form you fill in, a dialog. If a card only frames text, remove it.
- **Grouped rows** (settings, members, keys) use `settings-section.svelte`: a small heading, optional actions, and one divided surface.
- **Form dialogs** use `form-dialog.svelte` with a `FormAction` (`#lib/form-action.svelte`) for the busy flag and error line. Reset fields in `onclosed`, not in an `$effect`.
- **Sections with sub-pages** (Settings, Organization) use `section-layout.svelte`: page heading, an identity tile (`identity-tile.svelte`), then the `route-nav.svelte` icon list beside the content from `md` up, a horizontal scroller on mobile. Routes hold the state; items come from `#lib/nav.ts` (`settingsNavItemsFor`, `organizationNavItemsFor`). Any new section with sub-pages uses the same layout.
- **Loading data: in `load`, not in components.** SvelteKit preloads it on hover and swaps the page in complete, so routes never flash a skeleton and then jump.
  - Backend reads are prefetched into the QueryClient in `+page.ts` / `+layout.ts` (`queryClient.prefetchQuery(projectQueries.list(loadApi(fetch, url)))`) and read in the component with `createQuery(() => projectQueries.list())`: kept live by the `/api/live` socket. The app is client-only (static build), so `load` is also what keeps first paint free of half-loaded pages. Data that only exists after interaction (e.g. inside a dialog) uses `createQuery` with `enabled: open`, no prefetch.
  - Better Auth reads go through `getAuthJson` (`#lib/auth-api.ts`); refresh after a mutation with `invalidate('app:…')`.
- **Inline composers** beat side forms when adding to a list (see the task composer on the project detail page).

### Scrolling

No visible scrollbars. Every scroll container (`overflow-*-auto`/`-scroll`) gets `scroll-fade no-scrollbar` from `shadcn-svelte/tailwind.css` ([docs](https://www.shadcn-svelte.com/docs/utils/scroll-fade)): edges fade where more content waits. Use `scroll-fade-x` for horizontal scrollers. The document itself hides its scrollbar in `app.css`.

Exception: a container with sticky section headers keeps `no-scrollbar` but drops `scroll-fade`, which would wash out the pinned header.

### Motion vocabulary

| Motion                                                      | Tool                                    | Use                                                                            |
| ----------------------------------------------------------- | --------------------------------------- | ------------------------------------------------------------------------------ |
| `animate-rise` (fade + 10px rise + blur-in, 700ms expo-out) | Theme keyframe in `app.css`             | Hero/login entrance. Stagger with `[animation-delay:80ms]` steps, max 4 steps. |
| Route entrance (8px fly, 200ms)                             | `page-shell.svelte`                     | Every page body. Already built in.                                             |
| Route cross-fade (160ms view transition)                    | `(app)/+layout.svelte`, `app.css`       | Page region on every in-app navigation. Shell swaps instantly.                 |
| Count-up                                                    | `Tween` in `animated-number.svelte`     | Numeric KPIs.                                                                  |
| `animate:flip` + `transition:slide`                         | Svelte                                  | Rows that reorder or appear (tasks).                                           |
| `animate-pulse-dot`                                         | Theme keyframe                          | Live status dots only.                                                         |
| Hover nudge (caret/arrow `translate-x-0.5`)                 | Tailwind `group-hover`                  | Affordance on links and CTAs.                                                  |
| Sliding highlight                                           | `Spring` in `active-indicator.svelte`   | Tabs, single toggle groups, list selection.                                    |
| Panel enter (fade + 4px rise)                               | `tw-animate-css` on `data-state=active` | `Tabs.Content`.                                                                |

Every animation has a `motion-reduce:animate-none` (CSS) or a `prefersReducedMotion.current` check (Svelte). No exceptions.

### Light and dark

Both themes run through the `@a11y` contrast tests; check both when you change a token.

---

## Design principles

### 1. Keep the shell stable

Global navigation, workspace chrome, and section headers should stay visually stable while only the inner content changes.

Use SvelteKit layouts for this, not repeated page markup.

### 2. Use routes for information architecture

If a view should be deep-linkable, shareable, bookmarkable, or reload safely, it should have its own route.

Tabs can be used as the visual control for navigation, but the content should still come from subroutes when the state matters.

### 3. Prefer subtle motion

Motion should clarify:

- what changed
- where content came from
- what area is interactive

Avoid decorative animation that delays reading or makes the app feel noisy.

### 4. Use the right menu for the job

Do not force every section into the same navigation shape.

This app will likely need all of these:

- top navigation for broad product areas
- sidebar navigation for dense workspace flows
- breadcrumb navigation for deep object hierarchies
- tabs for sibling views inside one entity

### 5. Preserve reading flow

This product is content-heavy. Summaries, transcripts, chunk analysis, and review screens should prioritize:

- readable widths
- clear section hierarchy
- sticky context where useful
- minimal layout jumping

### 6. Prefer shadcn composition over custom UI

This repo should default to **shadcn-svelte-first** UI implementation.

- Always start from components in `apps/web/src/lib/components/ui/`.
- If an appropriate shadcn component exists upstream, add/install it before building a custom alternative.
- If no upstream shadcn component exists for the pattern, build a reusable local component instead of repeating custom styled markup in routes.
- Prefer `child` / render snippet composition when a shadcn component supports it.
- Do not replace available shadcn surfaces, inputs, dialogs, navigation, or form primitives with raw HTML.
- Raw HTML is acceptable for semantic document structure and content nested inside composed shadcn components.

---

## Recommended app layout hierarchy

### Root layout: app-wide shell

Keep the existing responsibility of `apps/web/src/routes/+layout.svelte`:

- app providers
- global header
- full-height shell
- scroll container for page content

This is the correct place for:

- the TanStack `QueryClientProvider`
- global theme state
- auth/session shell later
- mobile nav trigger later

### Nested layouts: section shells

As the app grows, use nested layouts for the major sections instead of building everything in the root layout.

Recommended direction:

```text
apps/web/src/routes/
  +layout.svelte                  # global providers + global shell
  (marketing)/
    +layout.svelte                # optional public/landing shell
    +page.svelte
  (app)/
    +layout.svelte                # authenticated workspace shell
    dashboard/+page.svelte
    meetings/
      +page.svelte                # meetings list
      [meetingId]/
        +layout.svelte            # meeting header + breadcrumb + tabs
        +page.svelte              # summary tab default route
        timeline/+page.svelte
        speakers/+page.svelte
        files/+page.svelte
```

### Why this structure works

- the **root layout** owns app-wide chrome
- the **workspace layout** owns dense navigation like a sidebar
- the **meeting layout** owns context for one meeting record
- the **child pages** stay small and focused

---

## Menu patterns

## 1. Top navigation

Use for broad, low-density sections.

Good for:

- Home
- Dashboard
- Meetings
- Templates
- Settings

Recommended component:

- start with the current `app-topbar.svelte`
- move to **shadcn-svelte `navigation-menu`** when the header needs grouped destinations or richer dropdowns

Use top navigation when:

- there are only a few primary destinations
- the user should always understand the overall app structure
- the section does not require many nested menu items

Do **not** use top navigation alone for dense workspace navigation once meetings, transcripts, runs, outputs, and review tools expand.

### shadcn-svelte note

`NavigationMenu` is best when the header has grouped links or richer dropdown content.

Install when needed:

```bash
bun x shadcn-svelte@latest add navigation-menu
```

Basic usage:

```svelte
<script lang="ts">
	import * as NavigationMenu from "#lib/components/ui/navigation-menu/index.js";
</script>

<NavigationMenu.Root>
	<NavigationMenu.List>
		<NavigationMenu.Item>
			<NavigationMenu.Trigger>Meetings</NavigationMenu.Trigger>
			<NavigationMenu.Content>
				<NavigationMenu.Link href="/meetings">All meetings</NavigationMenu.Link>
			</NavigationMenu.Content>
		</NavigationMenu.Item>
	</NavigationMenu.List>
</NavigationMenu.Root>
```

If custom anchors are needed, use the component's child/snippet pattern rather than replacing its internal structure.

---

## 2. Sidebar navigation

Use for the authenticated workspace once the app becomes more tool-like.

Good for:

- dashboard/workspace shell
- meeting collections
- jobs/runs/history
- admin/settings areas

Recommended component:

- **shadcn-svelte `sidebar`**

Install when needed:

```bash
bun x shadcn-svelte@latest add sidebar
```

Use sidebar navigation when:

- users spend long sessions in the product
- there are multiple work areas
- the app is more like a workspace than a marketing site

Recommended placement:

- top header stays thin
- sidebar owns section-level navigation
- main content area stays route-driven

The sidebar component is designed to live in a layout, which maps well to SvelteKit.

Example shape:

```svelte
<script lang="ts">
	import * as Sidebar from "#lib/components/ui/sidebar/index.js";
	import AppSidebar from "#lib/components/app-sidebar.svelte";

	let { children } = $props();
</script>

<Sidebar.Provider>
	<AppSidebar />

	<main class="min-w-0 flex-1">
		<Sidebar.Trigger />
		{@render children()}
	</main>
</Sidebar.Provider>
```

---

## 3. Breadcrumb navigation

Use breadcrumbs for deep drill-in paths.

Good for:

- `Meetings / Q1 planning / Speakers`
- `Templates / Management summary / Edit`
- `Runs / Job 42 / Chunk 7`

Recommended component:

- **shadcn-svelte `breadcrumb`**

Install when needed:

```bash
bun x shadcn-svelte@latest add breadcrumb
```

Breadcrumbs should supplement top nav or sidebar nav, not replace them.

Use them near the page title for orientation inside deep content.

---

## 4. Tabs

Use tabs for sibling views of the **same entity**.

Good for:

- Summary / Timeline / Speakers / Files
- Overview / Prompt / Output settings
- Run details / Logs / Metrics

Recommended component:

- **shadcn-svelte `tabs`**

Install when needed:

```bash
bun x shadcn-svelte@latest add tabs
```

Official composition is:

- `Tabs.Root`
- `Tabs.List`
- `Tabs.Trigger`
- `Tabs.Content`

Basic usage:

```svelte
<script lang="ts">
	import * as Tabs from "#lib/components/ui/tabs/index.js";
</script>

<Tabs.Root value="summary" class="w-full">
	<Tabs.List>
		<Tabs.Trigger value="summary">Summary</Tabs.Trigger>
		<Tabs.Trigger value="speakers">Speakers</Tabs.Trigger>
	</Tabs.List>

	<Tabs.Content value="summary">Summary content</Tabs.Content>
	<Tabs.Content value="speakers">Speakers content</Tabs.Content>
</Tabs.Root>
```

### Tabs rule for this app

Use `Tabs` for **local content switching inside a route**.

Use SvelteKit **subroutes** for major views that deserve their own URL.

That means:

- use `Tabs.Root` + `Tabs.Content` for local panels
- use nested layouts + child routes for linkable screens
- if route navigation should look like tabs, style normal links like tabs instead of repurposing an ARIA tabs widget for page navigation

#### Good use of `Tabs`

- a summary page switching between `Overview`, `Action items`, and `Risks`
- a settings page switching between `General` and `Advanced`
- a side panel switching between `Raw output` and `Rendered preview`

#### Good use of subroutes

- `Summary`, `Timeline`, `Speakers`, and `Files` as real meeting views
- template editor sections with their own loaders/actions
- run detail screens with shareable URLs

---

## Layouts + subroutes + Tabs: recommended pattern

This is the preferred pattern for meeting detail pages.

### Route structure

```text
apps/web/src/routes/(app)/meetings/[meetingId]/
  +layout.svelte
  +page.svelte              # default tab: summary
  timeline/+page.svelte
  speakers/+page.svelte
  files/+page.svelte
```

### Why this is the right pattern

- the meeting header stays mounted
- the tab bar stays mounted
- each tab is a real route
- deep-linking works
- reloading works
- each child page can fetch only what it needs

### Example meeting layout with subroute navigation

```svelte
<script lang="ts">
	import { page } from "$app/state";
	import * as Breadcrumb from "#lib/components/ui/breadcrumb/index.js";

	let { children } = $props();

	const basePath = $derived(`/meetings/${page.params.meetingId}`);
	const items = $derived([
		{ href: basePath, label: "Summary" },
		{ href: `${basePath}/timeline`, label: "Timeline" },
		{ href: `${basePath}/speakers`, label: "Speakers" },
		{ href: `${basePath}/files`, label: "Files" }
	]);

	function isActive(href: string) {
		return page.url.pathname === href;
	}
</script>

<div class="mx-auto flex w-full max-w-6xl flex-col gap-6 px-4 py-6">
	<Breadcrumb.Root>
		<Breadcrumb.List>
			<Breadcrumb.Item>
				<Breadcrumb.Link href="/meetings">Meetings</Breadcrumb.Link>
			</Breadcrumb.Item>
			<Breadcrumb.Separator />
			<Breadcrumb.Item>
				<Breadcrumb.Page>Current meeting</Breadcrumb.Page>
			</Breadcrumb.Item>
		</Breadcrumb.List>
	</Breadcrumb.Root>

	<header class="flex flex-col gap-2">
		<h1 class="text-2xl font-semibold">Meeting title</h1>
		<p class="text-sm text-muted-foreground">
			Transcript, summary, and analysis views for one meeting.
		</p>
	</header>

	<nav class="border-b">
		<ul class="flex flex-wrap gap-2">
			{#each items as item (item.href)}
				<li>
					<a
						href={item.href}
						class={`inline-flex rounded-t-md border border-b-0 px-3 py-2 text-sm transition-colors ${
							isActive(item.href)
								? "border-border bg-card text-foreground"
								: "border-transparent text-muted-foreground hover:text-foreground"
						}`}
					>
						{item.label}
					</a>
				</li>
			{/each}
		</ul>
	</nav>

	<section>
		{@render children()}
	</section>
</div>
```

### Example local `Tabs` usage inside the default meeting summary route

`apps/web/src/routes/(app)/meetings/[meetingId]/+page.svelte`

```svelte
<script lang="ts">
	import * as Tabs from "#lib/components/ui/tabs/index.js";
</script>

<Tabs.Root value="overview" class="w-full">
	<Tabs.List class="w-full justify-start">
		<Tabs.Trigger value="overview">Overview</Tabs.Trigger>
		<Tabs.Trigger value="actions">Action items</Tabs.Trigger>
		<Tabs.Trigger value="risks">Risks</Tabs.Trigger>
	</Tabs.List>

	<Tabs.Content value="overview" class="pt-4">
		<p class="text-sm text-muted-foreground">Management-ready summary content.</p>
	</Tabs.Content>

	<Tabs.Content value="actions" class="pt-4">
		<p class="text-sm text-muted-foreground">Action items extracted from the meeting.</p>
	</Tabs.Content>

	<Tabs.Content value="risks" class="pt-4">
		<p class="text-sm text-muted-foreground">Open questions, blockers, and risk signals.</p>
	</Tabs.Content>
</Tabs.Root>
```

### Practical note

This pattern keeps responsibilities clear:

- the nested `+layout.svelte` owns breadcrumb + subroute navigation
- the child route owns the actual screen content
- `Tabs` still exist, but only where the UI is switching local panels inside one route

---

## Suggested layout patterns by screen type

## 1. Public / lightweight pages

Examples:

- landing page
- sign-in page
- about/help pages

Pattern:

- top navigation only
- centered content width
- minimal chrome

Use:

- `navigation-menu` for the top bar
- no sidebar
- limited motion

---

## 2. Workspace home / dashboard

Examples:

- dashboard
- meetings list
- recent runs

Pattern:

- sidebar + top bar
- large content area
- cards, tables, filters

Use:

- `sidebar`
- `breadcrumb` only if depth increases
- optional `tabs` inside cards or detail panes

---

## 3. Detail workspace pages

Examples:

- meeting detail
- template detail
- run detail

Pattern:

- breadcrumb near title
- sticky local header if useful
- subroute navigation near the title, optionally styled like tabs

Use:

- `breadcrumb`
- `tabs`
- nested `+layout.svelte`

This will likely be the dominant pattern for the real product.

---

## 4. Long-form reading/review pages

Examples:

- transcript review
- final summary review
- chunk-by-chunk QA

Pattern:

- stable outer shell
- reading column with good line length
- sticky metadata/actions
- avoid cramped, dashboard-like density

Use:

- sidebar only for global navigation
- local page header
- tabs only if there are a few sibling reading modes

---

## Svelte-native animation guidance

Prefer native Svelte motion before custom animation libraries.

### Use `transition:` for enter/exit

Recommended primitives:

- `fade` for subtle appearance/disappearance
- `fly` for small contextual movement
- `slide` for expanding/collapsing sections
- `scale` only for compact overlays or popovers

Good uses in this app:

- mobile menu opening
- inline alerts
- empty states appearing
- filter panels expanding
- route content changing inside a stable shell

Example:

```svelte
<script lang="ts">
	import { prefersReducedMotion } from "svelte/motion";
	import { fade, fly } from "svelte/transition";

	let open = $state(false);
</script>

{#if open}
	<div
		in:fly={{ y: prefersReducedMotion.current ? 0 : 8, duration: 180 }}
		out:fade={{ duration: 120 }}
	>
		Animated panel
	</div>
{/if}
```

### Use `animate:flip` for reordering lists

Use this when rows/cards change position because of:

- sorting
- filtering
- status changes
- drag-and-drop later

Example:

```svelte
<script lang="ts">
	import { flip } from "svelte/animate";

	let items = $state([
		{ id: "1", label: "Chunk 1" },
		{ id: "2", label: "Chunk 2" }
	]);
</script>

{#each items as item (item.id)}
	<div animate:flip>
		{item.label}
	</div>
{/each}
```

### Use `Spring` or `Tween` for value motion

Use `svelte/motion` when the thing that changes is a value, not a DOM mount/unmount.

Good uses:

- progress meters
- analysis progress percentages
- panel resize indicators
- subtle drag position or scrubber feedback

Prefer the modern Svelte 5 classes:

- `Spring`
- `Tween`

Avoid older `spring()` / `tweened()` unless maintaining legacy code.

### Always respect reduced motion

Use `prefersReducedMotion.current` to reduce or remove movement.

This is especially important for:

- route transitions
- sidebar movement
- large panel shifts
- repeated animations inside review flows

---

## Motion rules for this app

### Good motion

- 120ms to 220ms for most UI transitions
- small distance movement only
- fade + slight fly for local content
- slide for disclosure sections
- flip for reordered lists

### Avoid

- full-page dramatic transitions
- large parallax movement
- delayed content reveal that blocks reading
- bouncing animations on frequently updated content (the one exception: instrument needles settle with a small overshoot)
- animating every panel on every route change

### Specific recommendation

Animate the **inner content**, not the whole shell.

The header/sidebar/breadcrumbs should feel stable. Only the changing content region should move.

---

## Styling and spacing guidance

### Page widths

Every page uses the one `page-shell.svelte` width (`max-w-7xl`, left edge shared), so headings never jump between routes. Do not add per-page caps. Cap a single block instead (e.g. a long text column at `max-w-prose`) when line length matters.

Working screens (pane layouts, see **Workspace layout**) are the exception: they fill the panel edge to edge.

### Section spacing

Default vertical rhythm should feel roomy.

Good defaults:

- `gap-4` for small grouped controls
- `gap-6` for section spacing
- `gap-8` for page-level blocks

### Borders and surfaces

See **Visual language → Surfaces**. In short:

- one raised panel, one list surface per section
- hairline borders from `--border` only, never custom opacities
- avoid nested outlines; a bordered list inside a card is one border too many

The visual hierarchy should come more from spacing and typography than from heavy decoration.

---

## Typography and readability

This app is primarily a reading and review interface, so typography matters more than ornamental UI.

### Priorities

- strong heading hierarchy for scanability
- comfortable paragraph line length for summaries
- easy differentiation between metadata and primary content
- visible but quiet timestamps, speaker labels, and system status text

### Recommended usage

- page title: clear, compact, high contrast
- section headings: consistent and scannable
- body copy: avoid ultra-small text for transcript or summary content
- metadata: `text-muted-foreground`, still 4.5:1 against every surface

For transcript-heavy screens, prefer a dedicated reading column over packing everything into bordered cards.

---

## Responsive strategy

The desktop and mobile shells should not be identical.

### Desktop

- sidebar for dense navigation
- breadcrumb + local section nav near the title
- multi-column layouts only where comparison actually helps

### Mobile

- collapse sidebar into a `sheet` or drawer-style menu
- keep breadcrumb short or partially collapsed
- allow local tab bars to wrap or scroll horizontally
- stack dense metadata blocks vertically

If a horizontal tab/nav row becomes cramped, prefer:

1. horizontal scrolling
2. wrapping when labels are short
3. a `select` fallback only when the list gets too large

---

## Loading and empty states

Because many screens depend on backend data, loading states should be designed up front.

### Use skeletons when layout is known

Examples:

- meeting list rows
- summary cards
- transcript blocks
- metadata side panels

### Use spinners only for small indeterminate actions

Examples:

- button-level submit states
- retry actions
- short background refresh states

### Empty states should be actionable

Bad empty state:

- “No data.”

Good empty state:

- explain why the screen is empty
- say what the user can do next
- keep the layout stable so the page does not jump after loading

---

## Recommended shadcn-svelte install list for the next phase

When the UI starts expanding beyond the current POC, these are the first components worth adding:

```bash
bun x shadcn-svelte@latest add tabs navigation-menu sidebar breadcrumb sheet separator
```

Likely follow-ups after that:

- `button`
- `card`
- `input`
- `textarea`
- `select`
- `dialog`
- `dropdown-menu`
- `scroll-area`

---

## Implementation guidance for this repo

### Near-term

1. Keep `apps/web/src/routes/+layout.svelte` as the global shell.
2. Keep `app-topbar.svelte` thin (trigger, breadcrumb, status) until more destinations exist.
3. Introduce route groups and nested layouts before the app gets many pages.
4. Use nested subroutes for meeting/template/run detail screens, with link bars styled like tabs where helpful.

### Medium-term

1. Add a workspace route group such as `(app)`.
2. Move dense product navigation into `sidebar`.
3. Add `breadcrumb` + subroute navigation to detail pages, and use local `Tabs` inside the child views where needed.
4. Use subtle motion only inside changing content regions.

---

## Final rule of thumb

- **Layouts** own persistent chrome.
- **Routes** own meaningful screen state.
- **Tabs** present local sibling panels; meaningful screen state should live in subroutes.
- **Sidebar/top nav/breadcrumbs** each solve different navigation problems.
- **Animations** should guide attention, never compete with the content.
- **Decoration** (grid, spotlight, gradient text, glow) belongs to the hero and auth screens only.
