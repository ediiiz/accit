<script lang="ts">
	import { Button } from '#lib/components/ui/button/index.js';
	import { m } from '#lib/paraglide/messages.js';
	import type { PortalApp } from '#lib/api.js';

	let { apps }: { apps: PortalApp[] } = $props();

	const links = $derived(apps.filter((app) => app.url.startsWith('https://')));
</script>

{#if links.length > 0}
	<nav aria-label={m.apps()}>
		<ul class="flex flex-col gap-2">
			{#each links as app (app.url)}
				<li>
					<Button variant="outline" class="h-11 w-full text-base" href={app.url} rel="noreferrer">
						{app.name}
					</Button>
				</li>
			{/each}
		</ul>
	</nav>
{/if}
