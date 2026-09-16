<script lang="ts">
	import type { WordEntry } from '$lib/api';
	import HighlightedText from './HighlightedText.svelte';

	/** Read-only review browse card. No learning side effects: unlike WordView it
	 * never records encounters, comprehension levels, notes or bookmarks. */
	let { entry }: { entry: WordEntry } = $props();
</script>

<article class="browse-card">
	<header class="head">
		<h2>{entry.display}</h2>
		<div class="meta">
			{#if entry.phonetic}<span class="phonetic">{entry.phonetic}</span>{/if}
			{#if entry.frequency_rank}<span class="rank">#{entry.frequency_rank}</span>{/if}
		</div>
	</header>
	{#if entry.senses.length}
		{#each entry.senses as sense, i}
			<section class="sense">
				<div class="sense-number">{String(i + 1).padStart(2, '0')}</div>
				<div class="sense-body">
					<div class="sense-tags">
						{#if sense.pos}{sense.pos}{/if}{#if sense.level}<span class="badge">{sense.level}</span>{/if}
					</div>
					{#if sense.english_definition}<p class="reading english">{sense.english_definition}</p>{/if}
					{#if sense.chinese_definition}<p class="chinese">{sense.chinese_definition}</p>{/if}
					{#each sense.examples as ex}
						<blockquote>
							<p class="reading"><HighlightedText text={ex.text} word={entry.word} /></p>
							{#if ex.translation}<footer>{ex.translation}</footer>{/if}
						</blockquote>
					{/each}
				</div>
			</section>
		{/each}
	{:else}
		<p class="muted">No definitions available.</p>
	{/if}
	{#if entry.collocations.length}
		<section class="extra">
			<h3>Collocations</h3>
			{#each entry.collocations as c}
				<div class="collocation"><span>{c.text}</span><small>{c.gloss ?? ''}</small></div>
			{/each}
		</section>
	{/if}
	{#if entry.synonyms.length || entry.antonyms.length}
		<section class="extra related-groups">
			{#if entry.synonyms.length}
				<div><h3>Synonyms</h3><p>{entry.synonyms.join(' · ')}</p></div>
			{/if}
			{#if entry.antonyms.length}
				<div><h3>Antonyms</h3><p>{entry.antonyms.join(' · ')}</p></div>
			{/if}
		</section>
	{/if}
</article>

<style>
	.head {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: 12px;
		flex-wrap: wrap;
	}
	.head h2 {
		font: 450 40px/1.1 var(--serif);
		letter-spacing: -0.025em;
		margin: 0 0 4px;
	}
	.meta {
		display: flex;
		align-items: center;
		gap: 10px;
	}
	.phonetic {
		color: var(--muted);
		font-size: 14px;
	}
	.rank {
		font-size: 10px;
		color: var(--muted);
		border: 1px solid var(--line);
		border-radius: 4px;
		padding: 2px 6px;
	}
	.sense {
		display: flex;
		gap: 16px;
		margin: 22px 0;
	}
	.sense:first-of-type {
		margin-top: 18px;
	}
	.sense:last-of-type {
		margin-bottom: 0;
	}
	.sense-number {
		font: italic 500 17px var(--serif);
		color: var(--muted);
		padding-top: 3px;
	}
	.sense-body {
		min-width: 0;
	}
	.sense-tags {
		font: italic 500 13px var(--serif);
		color: var(--accent);
		display: flex;
		align-items: center;
		gap: 12px;
	}
	.english {
		font: 450 18px/1.7 var(--serif);
		white-space: pre-line;
		margin: 8px 0;
	}
	.chinese {
		font-size: 13px;
		color: var(--muted);
		white-space: pre-line;
		margin: 6px 0 16px;
	}
	blockquote {
		margin: 14px 0;
		border-left: 2px solid var(--line);
		padding: 3px 0 3px 16px;
	}
	blockquote p {
		font: 450 15px/1.8 var(--serif);
		margin: 0;
	}
	blockquote footer {
		font-size: 11px;
		color: var(--muted);
		margin-top: 5px;
	}
	.extra {
		border-top: 1px solid var(--line);
		padding-top: 16px;
		margin-top: 20px;
	}
	.extra h3 {
		font-size: 12px;
		color: var(--muted);
		margin-bottom: 8px;
	}
	.collocation {
		display: flex;
		justify-content: space-between;
		gap: 12px;
		padding: 6px 0;
	}
	.collocation span {
		font: 500 15px var(--serif);
	}
	.collocation small {
		font-size: 11px;
		color: var(--muted);
	}
	.related-groups {
		display: flex;
		gap: 28px;
		flex-wrap: wrap;
	}
	.related-groups p {
		font: 500 15px/1.7 var(--serif);
		margin: 0;
	}
</style>
