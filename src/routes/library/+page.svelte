<script lang="ts">
	import { onDestroy } from 'svelte';
	import {
		libraryWords,
		setWordIgnored,
		errorMessage,
		statusLabel,
		dueLabel,
		type LibraryPage
	} from '$lib/api';
	import Icon from '$lib/components/Icon.svelte';
	let query = $state('');
	let filter = $state('all');
	let offset = $state(0);
	let data = $state<LibraryPage>({ words: [], total: 0 });
	let loading = $state(true);
	let error = $state('');
	let busy = $state<number | null>(null);
	let generation = 0;
	const filters = [
		['all', 'All'],
		['bookmarked', 'Saved'],
		['learning', 'Learning'],
		['familiar', 'Familiar'],
		['stable', 'Stable'],
		['ignored', 'Paused']
	];
	async function load(q = query, f = filter, o = offset, token = ++generation) {
		loading = true;
		error = '';
		try {
			const rows = await libraryWords(q, f, o);
			if (token === generation) data = rows;
		} catch (e) {
			if (token === generation) error = errorMessage(e);
		} finally {
			if (token === generation) loading = false;
		}
	}
	$effect(() => {
		const q = query,
			f = filter,
			o = offset;
		const token = ++generation;
		loading = true;
		const timer = setTimeout(() => load(q, f, o, token), 150);
		return () => clearTimeout(timer);
	});
	onDestroy(() => {
		++generation;
	});
	async function restore(id: number) {
		if (busy !== null) return;
		busy = id;
		try {
			await setWordIgnored(id, false);
			await load();
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = null;
		}
	}
</script>

<main class="page">
	<div class="library-top">
		<h1>Words <span class="count">{loading ? '' : data.total}</span></h1>
		<div class="filter-search">
			<Icon name="search" size={16} /><input
				aria-label="Search words and notes"
				placeholder="Filter words or notes…"
				bind:value={query}
				oninput={() => (offset = 0)}
			/>
		</div>
	</div>
	<div class="filters">
		{#each filters as [key, label]}<button
				class:chosen={filter === key}
				aria-pressed={filter === key}
				onclick={() => {
					filter = key;
					offset = 0;
				}}>{label}</button
			>{/each}
	</div>
	{#if error}<div class="error" role="alert">
			{error} <button onclick={() => load()}>Retry</button>
		</div>
	{:else if loading}<div class="skeleton" aria-label="Loading words" role="status"></div>
	{:else if !data.words.length}<div class="empty">
			<div class="empty-icon">
				<Icon name={filter === 'bookmarked' ? 'bookmark' : 'book'} size={26} />
			</div>
			<h2>{query ? 'No matches' : filter === 'all' ? 'No words yet' : 'Nothing here'}</h2>
			<p>
				{query
					? 'Try another spelling, or search your notes.'
					: filter === 'bookmarked'
						? 'Bookmark an entry to keep it here.'
						: 'Every word you look up is collected automatically.'}
			</p>
			{#if query || filter !== 'all'}<button
					onclick={() => {
						query = '';
						filter = 'all';
						offset = 0;
					}}>Clear filters</button
				>{:else}<a href="/" class="button primary">Look up a word <Icon name="arrow" size={16} /></a
				>{/if}
		</div>
	{:else}<div class="word-list">
			{#each data.words as word (word.id)}<div class="word-row">
					<a class="word-link" href={`/?word=${encodeURIComponent(word.word)}`}
						><div class="word-title">
							<strong>{word.display}</strong>{#if word.bookmarked}<Icon
									name="bookmark"
									size={13}
								/>{/if}<span>{word.phonetic ?? ''}</span>
						</div>
						<p>{word.definition}</p>
						{#if word.note}<small class="note">{word.note}</small>{/if}</a
					>
					<div class="word-status">
						<span class="badge" class:paused={word.status === 'ignored'}
							>{statusLabel[word.status] ?? word.status}</span
						><small
							>{word.status === 'ignored'
								? 'reminders off'
								: dueLabel(word.next_review_at) || `${word.visit_count}×`}</small
						>{#if word.status === 'ignored'}<button
								class="restore"
								disabled={busy !== null}
								onclick={() => restore(word.id)}>{busy === word.id ? '…' : 'Resume'}</button
							>{/if}
					</div>
				</div>{/each}
		</div>
	{/if}
	{#if data.total > 24}<div class="pagination">
			<span>{Math.floor(offset / 24) + 1} / {Math.ceil(data.total / 24)}</span>
			<div>
				<button
					disabled={offset === 0 || loading}
					onclick={() => (offset = Math.max(0, offset - 24))}>Previous</button
				><button disabled={offset + 24 >= data.total || loading} onclick={() => (offset += 24)}
					>Next</button
				>
			</div>
		</div>{/if}
</main>

<style>
	.library-top {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 16px;
		margin-bottom: 20px;
	}
	.library-top h1 {
		margin: 0;
		font-size: 24px;
	}
	.count {
		font: 500 15px var(--serif);
		color: var(--muted);
		margin-left: 8px;
	}
	.filter-search {
		position: relative;
		max-width: 260px;
		flex: 1;
	}
	.filter-search :global(svg) {
		position: absolute;
		left: 13px;
		top: 13px;
		color: var(--muted);
	}
	.filter-search input {
		width: 100%;
		padding-left: 38px;
		font-size: 12px;
	}
	.filters {
		display: flex;
		gap: 4px;
		flex-wrap: wrap;
		border-bottom: 1px solid var(--line);
		padding-bottom: 14px;
	}
	.filters button {
		font-size: 12px;
		border: 0;
		background: transparent;
		color: var(--muted);
		padding: 6px 12px;
	}
	.filters button.chosen {
		color: var(--accent);
		background: var(--soft);
	}
	.word-row {
		display: flex;
		gap: 22px;
		align-items: center;
		padding: 20px 0;
		border-bottom: 1px solid var(--line);
	}
	.word-link {
		flex: 1;
		min-width: 0;
		text-decoration: none;
		color: var(--fg);
	}
	.word-title {
		display: flex;
		align-items: center;
		gap: 10px;
		flex-wrap: wrap;
	}
	.word-title strong {
		font: 500 21px var(--serif);
		transition: color 0.16s var(--ease);
	}
	.word-title :global(svg) {
		color: var(--accent);
	}
	.word-title span {
		font-size: 11px;
		color: var(--muted);
	}
	.word-link p {
		font-size: 12px;
		color: var(--muted);
		margin: 6px 0 0;
		overflow: hidden;
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
		white-space: pre-line;
	}
	.word-link:hover strong {
		color: var(--accent);
	}
	.note {
		display: block;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--accent);
		font-size: 11px;
		margin-top: 6px;
	}
	.word-status {
		display: flex;
		flex-direction: column;
		gap: 6px;
		align-items: flex-end;
	}
	.word-status small {
		font-size: 10px;
		color: var(--muted);
	}
	.paused {
		background: var(--warm);
		color: var(--muted);
	}
	.restore {
		padding: 4px 8px;
		font-size: 10px;
	}
	.pagination {
		display: flex;
		align-items: center;
		justify-content: space-between;
		font-size: 11px;
		color: var(--muted);
		padding-top: 20px;
	}
	.pagination > div {
		display: flex;
		gap: 8px;
	}
	.pagination button {
		font-size: 11px;
		padding: 7px 12px;
	}
	@media (max-width: 600px) {
		.library-top {
			align-items: flex-start;
			flex-direction: column;
		}
		.filter-search {
			max-width: none;
			width: 100%;
		}
		.filters button {
			font-size: 11px;
			padding: 6px 9px;
		}
		.word-row {
			gap: 12px;
		}
		.word-title span {
			width: 100%;
		}
	}
</style>
