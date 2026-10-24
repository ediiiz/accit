<script lang="ts">
	import CircleCheck from '@lucide/svelte/icons/circle-check';
	import Clock from '@lucide/svelte/icons/clock';
	import House from '@lucide/svelte/icons/house';
	import LoaderCircle from '@lucide/svelte/icons/loader-circle';
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
	import { onMount, type Component } from 'svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import AppLinks from './app-links.svelte';
	import RevokeDialog from './revoke-dialog.svelte';
	import Troubleshooting from './troubleshooting.svelte';
	import { m } from '#lib/paraglide/messages.js';
	import { overwriteGetLocale } from '#lib/paraglide/runtime.js';
	import type { PortalController } from '#lib/portal-controller.svelte.js';
	import { formatUntil, localeFor, type Screen } from '#lib/portal.js';

	let {
		portal,
		languages = navigator.languages
	}: { portal: PortalController; languages?: readonly string[] } = $props();

	// Messages read this, so a status that brings the admin's default language re-renders them.
	const locale = $derived(localeFor(languages, portal.status?.default_language));

	overwriteGetLocale(() => locale);

	$effect(() => {
		document.documentElement.lang = locale;
	});

	interface Copy {
		icon: Component;
		heading: string;
		body?: string;
	}

	const copies: Partial<Record<Screen, () => Copy>> = {
		not_enabled: () => ({ icon: Clock, heading: m.not_enabled_heading(), body: m.not_enabled_body() }),
		submitting: () => ({ icon: LoaderCircle, heading: m.submitting_heading() }),
		enabled: () => ({
			icon: CircleCheck,
			heading: m.enabled_heading(),
			body: portal.status?.expiry
				? m.enabled_body({ until: formatUntil(portal.status.expiry, locale) })
				: undefined
		}),
		expired: () => ({ icon: Clock, heading: m.expired_heading() }),
		changed: () => ({ icon: TriangleAlert, heading: m.changed_heading(), body: m.changed_body() }),
		home: () => ({ icon: House, heading: m.home_heading(), body: m.home_body() }),
		unavailable: () => ({
			icon: TriangleAlert,
			heading:
				portal.failedAction === 'revoke' ? m.unavailable_revoke_heading() : m.unavailable_heading(),
			body: m.unavailable_body()
		}),
		rate_limited: () => ({ icon: Clock, heading: m.rate_limited_heading() })
	};

	const copy = $derived(copies[portal.screen]?.() ?? null);

	const failed = $derived(portal.screen === 'unavailable' || portal.screen === 'rate_limited');

	const action = $derived.by(() => {
		switch (portal.screen) {
			case 'not_enabled':
			case 'expired':
			case 'changed':
				return { label: m.enable_access(), disabled: false, run: () => portal.unlock() };
			case 'submitting':
				return { label: m.enable_access(), disabled: true, run: () => portal.unlock() };
			case 'enabled':
				return { label: m.extend_access(), disabled: false, run: () => portal.unlock() };
			case 'unavailable':
				return { label: m.try_again(), disabled: false, run: () => portal.retry() };
			case 'rate_limited':
				return {
					label:
						portal.secondsLeft > 0
							? m.try_again_in({ seconds: portal.secondsLeft })
							: m.try_again(),
					disabled: portal.secondsLeft > 0,
					run: () => portal.retry()
				};
			default:
				return null;
		}
	});

	const showTip = $derived(['not_enabled', 'expired', 'changed', 'enabled'].includes(portal.screen));

	onMount(() => {
		void portal.load();

		return () => portal.destroy();
	});

	function refresh() {
		if (document.visibilityState === 'visible') {
			void portal.load();
		}
	}
</script>

<svelte:window onfocus={refresh} />
<svelte:document onvisibilitychange={refresh} />

<svelte:head>
	<title>{portal.status?.title ?? m.portal_title()}</title>
</svelte:head>

{#snippet message(text: Copy)}
	<div class="flex items-start gap-3">
		<text.icon
			aria-hidden="true"
			class={['mt-1 size-6 shrink-0', text.icon === LoaderCircle && 'motion-safe:animate-spin']}
		/>
		<div class="flex flex-col gap-1">
			<h1 class="text-2xl font-semibold">{text.heading}</h1>
			{#if text.body}
				<p class="text-muted-foreground">{text.body}</p>
			{/if}
		</div>
	</div>
{/snippet}

<main class="mx-auto flex min-h-dvh w-full max-w-md flex-col justify-center gap-6 px-4 py-10">
	<div role="status" class="empty:hidden">
		{#if copy && !failed}
			{@render message(copy)}
		{/if}
	</div>
	<div role="alert" class="empty:hidden">
		{#if copy && failed}
			{@render message(copy)}
		{/if}
	</div>

	{#if action}
		<Button
			class="h-11 w-full text-base"
			disabled={action.disabled}
			aria-busy={portal.screen === 'submitting' ? 'true' : undefined}
			onclick={action.run}
		>
			{action.label}
		</Button>
	{/if}

	{#if portal.status && (portal.screen === 'enabled' || portal.screen === 'home')}
		<AppLinks apps={portal.status.apps} />
	{/if}

	{#if portal.screen === 'enabled'}
		<RevokeDialog busy={portal.revoking} onconfirm={() => portal.revoke()} />
	{/if}

	{#if showTip}
		<p class="text-sm text-muted-foreground">{m.wifi_tip()}</p>
	{/if}

	{#if portal.status}
		<Troubleshooting address={portal.status.address} family={portal.status.family} />
	{/if}
</main>
