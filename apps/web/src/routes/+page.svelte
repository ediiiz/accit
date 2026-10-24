<script lang="ts">
	import { createApi } from '#lib/api.js';
	import Portal from '#lib/components/portal/portal.svelte';
	import { PortalController, browserLastUnlocked } from '#lib/portal-controller.svelte.js';

	const portal = new PortalController(createApi(), browserLastUnlocked);
</script>

<Portal {portal} />
