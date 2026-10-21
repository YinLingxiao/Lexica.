<script lang="ts">
	import { splitSyllables } from '$lib/syllables';

	let { word, split = true }: { word: string; split?: boolean } = $props();

	const parts = $derived(split ? splitSyllables(word) : [word]);
</script>

{#key word}<span class="syllable-headword" aria-label={word}
		>{#each parts as part, i}{#if i > 0}<span
					class="syllable-dot"
					style:--i={i - 1}
					aria-hidden="true">·</span
				>{/if}<span aria-hidden="true">{part}</span>{/each}</span
	>{/key}

<style>
	.syllable-headword {
		display: inline;
	}
	.syllable-dot {
		display: inline-block;
		width: 0.3em;
		margin: 0 0.04em;
		text-align: center;
		overflow: clip;
		vertical-align: baseline;
		font-weight: 400;
		color: color-mix(in srgb, var(--accent) 65%, var(--kraft));
		animation: syllable-split 640ms var(--ease) calc(380ms + var(--i, 0) * 80ms) backwards;
	}
	@keyframes syllable-split {
		from {
			width: 0;
			margin: 0;
			opacity: 0;
			transform: translateY(0.18em) scale(0.3);
		}
		60% {
			opacity: 1;
		}
	}
</style>
