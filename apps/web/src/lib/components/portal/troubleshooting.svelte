<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';

	let { address, family }: { address: string; family: 'ipv4' | 'ipv6' } = $props();
</script>

<details class="text-sm">
	<summary
		class="min-h-11 cursor-pointer py-2 text-muted-foreground focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none"
	>
		{m.troubleshooting()}
	</summary>
	<dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 pt-1">
		<dt class="text-muted-foreground">{m.address()}</dt>
		<dd class="font-mono break-all">{address}</dd>
		<dt class="text-muted-foreground">{m.family()}</dt>
		<dd>{family === 'ipv6' ? 'IPv6' : 'IPv4'}</dd>
	</dl>
</details>
