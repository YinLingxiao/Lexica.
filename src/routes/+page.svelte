<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import SearchBox from '$lib/components/SearchBox.svelte';
	import WordView from '$lib/components/WordView.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import {
		lookupWord,
		recentWords,
		statsSummary,
		appInfo,
		relTime,
		errorMessage,
		type LookupWordResult,
		type RecentWord,
		type StatsSummary
	} from '$lib/api';
	let result = $state<LookupWordResult | null>(null);
	let value = $state('');
	let recent = $state<RecentWord[]>([]);
	let stats = $state<StatsSummary | null>(null);
	let wordCount = $state<number | null>(null);
	let error = $state('');
	let loading = $state(false);
	let request = 0;
	const query = $derived(page.url.searchParams.get('word') ?? '');
	async function refresh() {
		const results = await Promise.allSettled([recentWords(8), statsSummary()]);
		if (results[0].status === 'fulfilled') recent = results[0].value;
		if (results[1].status === 'fulfilled') stats = results[1].value;
	}
	async function load(word: string) {
		const token = ++request;
		loading = true;
		error = '';
		result = null;
		value = word;
		try {
			const found = await lookupWord(word);
			if (token === request) result = found;
		} catch (e) {
			if (token === request) error = errorMessage(e);
		} finally {
			if (token === request) {
				loading = false;
				void refresh();
			}
		}
	}
	function doLookup(word: string) {
		const q = word.trim();
		if (!q) return;
		if (q === query) {
			// Keep the active entry and any unsaved note when searching the same word again.
			if (!result && !loading) void load(q);
			return;
		}
		void goto(`/?word=${encodeURIComponent(q)}`, { noScroll: true });
	}
	$effect(() => {
		if (query) void load(query);
		else {
			++request;
			result = null;
			error = '';
			loading = false;
			value = '';
		}
	});
	onMount(() => {
		void refresh();
		void appInfo().then((info) => (wordCount = info.word_count));
	});
</script>

<main class="page home">
	{#if query}
		<a href="/" class="back"><Icon name="back" size={15} />Search</a>
		<SearchBox oncommit={doLookup} bind:value />
		<div class="word-area" aria-busy={loading}>
			{#if loading}<div class="skeleton"></div>{:else if error}<div class="error" role="alert">
					{error}
				</div>
				<button onclick={() => load(query)}>Try again</button
				>{:else if result}{#key result}<WordView
						{result}
						{query}
						oncommit={doLookup}
						ondismiss={() => goto('/')}
						onupdate={refresh}
					/>{/key}{/if}
		</div>
	{:else}
		<div class="hero">
			<SearchBox oncommit={doLookup} bind:value />
			<div class="hero-foot">
				<span>{wordCount === null ? '—' : `${wordCount.toLocaleString()} entries · offline`}</span>
				<span><kbd>/</kbd> to search</span>
			</div>
		</div>
		{#if stats}<div class="stat-row">
				{#each [{ label: 'Words', value: stats.encountered }, { label: 'Familiar', value: stats.familiar + stats.stable }, { label: 'Stable', value: stats.stable }] as m}<div
					>
						<span class="stat-label">{m.label}</span>
						<span class="stat-value">{m.value.toLocaleString()}</span>
					</div>{/each}
				<a class="due" class:ready={stats.due_now > 0} href="/review">
					<span class="stat-label">Due</span>
					<span class="stat-value"
						>{stats.due_now.toLocaleString()}<Icon name="arrow" size={15} /></span
					>
				</a>
			</div>{/if}
		{#if recent.length}<section class="recent">
				<div class="section-heading">
					<h2>Recent</h2>
					<a href="/library">All words</a>
				</div>
				<div class="recent-grid">
					{#each recent as r}<button onclick={() => doLookup(r.word)}
							><span><strong>{r.display}</strong><small>{r.phonetic ?? ''}</small></span><span
								class="when">{relTime(r.visited_at)}</span
							></button
						>{/each}
				</div>
			</section>
		{:else}
			<section class="recent">
				<div class="section-heading"><h2>Start here</h2></div>
				<div class="recent-grid">
					{#each ['meticulous', 'subtle', 'resilient', 'profound'] as word}<button
							onclick={() => doLookup(word)}><span><strong>{word}</strong></span></button
						>{/each}
				</div>
			</section>
		{/if}
	{/if}
</main>

<style>
	.home {
		max-width: 880px;
	}
	.hero {
		padding: 4vh 0 34px;
	}
	.hero-foot {
		display: flex;
		justify-content: space-between;
		color: var(--muted);
		font-size: 10px;
		padding: 12px 2px 0;
	}
	.stat-row {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
		gap: 1px;
		background: var(--line);
		border: 1px solid var(--line);
		border-radius: 12px;
		overflow: hidden;
	}
	.stat-row > * {
		background: var(--card);
		padding: 16px 18px;
		text-decoration: none;
	}
	.stat-label {
		display: block;
		font-size: 10px;
		letter-spacing: 1.2px;
		text-transform: uppercase;
		color: var(--muted);
	}
	.stat-value {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-top: 6px;
		font: 500 26px/1.1 var(--serif);
		color: var(--fg);
	}
	.due .stat-value {
		color: var(--muted);
	}
	.due.ready .stat-value {
		color: var(--accent);
	}
	.due :global(svg) {
		transition: transform 0.2s var(--ease);
	}
	.due:hover :global(svg) {
		transform: translateX(3px);
	}
	.recent {
		margin-top: 34px;
	}
	.recent-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0 28px;
	}
	.recent-grid button {
		border: 0;
		border-top: 1px solid var(--line);
		border-radius: 0;
		justify-content: space-between;
		padding: 13px 2px;
		text-align: left;
		background: none;
		min-width: 0;
	}
	.recent-grid button:hover {
		background: none;
	}
	.recent-grid button:hover strong {
		color: var(--accent);
	}
	.recent-grid strong {
		font: 500 19px var(--serif);
		transition: color 0.16s var(--ease);
	}
	.recent-grid small {
		font-size: 10px;
		color: var(--muted);
		display: block;
		margin-top: 4px;
	}
	.when {
		color: var(--muted);
		font-size: 10px;
		white-space: nowrap;
	}
	.back {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		font-size: 12px;
		text-decoration: none;
		color: var(--muted);
		margin-bottom: 20px;
	}
	.word-area {
		margin-top: 24px;
	}
	@media (max-width: 700px) {
		.stat-row {
			grid-template-columns: 1fr 1fr;
		}
		.recent-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
