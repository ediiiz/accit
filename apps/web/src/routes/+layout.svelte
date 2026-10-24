<script lang="ts">
	import '../app.css';
	import { getLocale, getTextDirection } from '#lib/paraglide/runtime.js';

	let { children } = $props();

	$effect(() => {
		const locale = getLocale();

		document.documentElement.lang = locale;
		document.documentElement.dir = getTextDirection(locale);
	});
</script>

{@render children()}
