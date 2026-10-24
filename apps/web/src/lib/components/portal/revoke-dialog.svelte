<script lang="ts">
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { m } from '#lib/paraglide/messages.js';

	let {
		busy,
		open = $bindable(false),
		trigger = true,
		onconfirm
	}: { busy: boolean; open?: boolean; trigger?: boolean; onconfirm: () => Promise<void> } =
		$props();

	async function confirm() {
		await onconfirm();
		open = false;
	}
</script>

<Dialog.Root bind:open>
	{#if trigger}
		<Dialog.Trigger>
			{#snippet child({ props })}
				<Button variant="ghost" class="h-11 w-full" {...props}>{m.disable_access()}</Button>
			{/snippet}
		</Dialog.Trigger>
	{/if}
	<Dialog.Content showCloseButton={false}>
		<Dialog.Header>
			<Dialog.Title>{m.disable_title()}</Dialog.Title>
		</Dialog.Header>
		<Dialog.Footer>
			<Button variant="outline" class="h-11" onclick={() => (open = false)}>{m.cancel()}</Button>
			<Button variant="destructive" class="h-11" disabled={busy} onclick={confirm}>
				{m.disable_confirm()}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
