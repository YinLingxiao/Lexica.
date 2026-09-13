<script lang="ts">
	let { text, word }: { text: string; word: string } = $props();
	const parts = $derived.by(() => {
		if (!word) return [text];
		const escaped = word.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
		return text.split(new RegExp(`(\\b${escaped}\\b)`, 'gi'));
	});
</script>

{#each parts as part, i}{#if i % 2}<mark>{part}</mark>{:else}{part}{/if}{/each}

<style>
	mark {
		color: var(--accent);
		background: transparent;
		font-weight: 600;
		text-decoration: underline;
		text-decoration-color: color-mix(in srgb, var(--accent) 30%, transparent);
		text-underline-offset: 4px;
		text-decoration-thickness: 2px;
	}
</style>
