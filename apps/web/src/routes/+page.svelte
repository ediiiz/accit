<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
</script>

<svelte:head>
	<title>{m.portal_title()}</title>
</svelte:head>

<main class="mx-auto flex min-h-dvh max-w-md flex-col justify-center gap-2 px-6 py-10">
	<h1 class="text-3xl font-semibold">{m.not_enabled_heading()}</h1>
	<p class="text-muted-foreground">{m.not_enabled_body()}</p>
</main>
