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
	import {
		copyFor,
		formatUntil,
		localeFor,
		primaryAction,
		type Copy,
		type Icon,
		type PrimaryAction
	} from '#lib/portal.js';

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

	const icons: Record<Icon, Component> = {
		check: CircleCheck,
		clock: Clock,
		home: House,
		spinner: LoaderCircle,
		alert: TriangleAlert
	};

	const copy = $derived(
		copyFor(portal.screen, {
			until: portal.status?.expiry ? formatUntil(portal.status.expiry, locale) : null,
			failedAction: portal.failedAction
		})
	);

	const failed = $derived(portal.screen === 'unavailable' || portal.screen === 'rate_limited');

	const action = $derived(primaryAction(portal.screen, portal.secondsLeft, portal.failedAction));

	let confirmingRevoke = $state(false);

	function run(command: PrimaryAction['command']) {
		if (command === 'revoke') {
			confirmingRevoke = true;

			return;
		}

		return command === 'unlock' ? portal.unlock() : portal.retry();
	}

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
	{@const Glyph = icons[text.icon]}
	<div class="flex items-start gap-3">
		<Glyph
			aria-hidden="true"
			class={['mt-1 size-6 shrink-0', text.icon === 'spinner' && 'motion-safe:animate-spin']}
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
	<div role="status" class="empty:sr-only">
		{#if copy && !failed}
			{@render message(copy)}
		{/if}
	</div>
	<div role="alert" class="empty:sr-only">
		{#if copy && failed}
			{@render message(copy)}
		{/if}
	</div>

	{#if action}
		<Button
			class="h-11 w-full text-base"
			disabled={action.disabled}
			aria-busy={portal.screen === 'submitting' ? 'true' : undefined}
			onclick={() => run(action.command)}
		>
			{action.label}
		</Button>
	{/if}

	{#if portal.status && (portal.screen === 'enabled' || portal.screen === 'home')}
		<AppLinks apps={portal.status.apps} />
	{/if}

	{#if portal.screen === 'enabled' || portal.failedAction === 'revoke'}
		<RevokeDialog
			busy={portal.revoking}
			trigger={portal.screen === 'enabled'}
			bind:open={confirmingRevoke}
			onconfirm={() => portal.revoke()}
		/>
	{/if}

	{#if showTip}
		<p class="text-sm text-muted-foreground">{m.wifi_tip()}</p>
	{/if}

	{#if portal.status}
		<Troubleshooting address={portal.status.address} family={portal.status.family} />
	{/if}
</main>
