<script lang="ts">
	import { onDestroy } from 'svelte';
	import { searchSuggest } from '$lib/api';
	import type { Suggestion } from '$lib/api';
	import Icon from './Icon.svelte';
	let { oncommit, value = $bindable('') }: { oncommit: (query: string) => void; value?: string } =
		$props();
	let inputEl: HTMLInputElement;
	let suggestions = $state<Suggestion[]>([]);
	let highlighted = $state(-1);
	let open = $state(false);
	let composing = false;
	let generation = 0;
	let timer: ReturnType<typeof setTimeout>;
	let blurTimer: ReturnType<typeof setTimeout>;
	let searchError = $state('');
	const id = $props.id();
	function schedule() {
		clearTimeout(timer);
		const token = ++generation;
		const query = value.trim();
		open = false;
		highlighted = -1;
		searchError = '';
		if (!query || composing) {
			suggestions = [];
			return;
		}
		timer = setTimeout(async () => {
			try {
				const rows = await searchSuggest(query, 8);
				if (token !== generation) return;
				suggestions = rows;
				open = rows.length > 0;
			} catch {
				if (token === generation) {
					suggestions = [];
					searchError = 'Suggestions unavailable — press Enter to look up.';
				}
			}
		}, 120);
	}
	function commit(word: string) {
		if (!word.trim()) return;
		++generation;
		clearTimeout(timer);
		open = false;
		value = word.trim();
		oncommit(value);
		inputEl?.blur();
	}
	function close() {
		++generation;
		clearTimeout(timer);
		open = false;
	}
	function key(e: KeyboardEvent) {
		if (composing || e.isComposing) return;
		if (e.key === 'ArrowDown' && open) {
			e.preventDefault();
			highlighted = Math.min(highlighted + 1, suggestions.length - 1);
		} else if (e.key === 'ArrowUp' && open) {
			e.preventDefault();
			highlighted = Math.max(highlighted - 1, -1);
		} else if (e.key === 'Enter') {
			e.preventDefault();
			commit(open && highlighted >= 0 ? suggestions[highlighted].word : value);
		} else if (e.key === 'Escape') {
			e.preventDefault();
			e.stopPropagation();
			if (open) close();
			else {
				value = '';
				schedule();
			}
		}
	}
	function globalKey(e: KeyboardEvent) {
		const target = e.target as HTMLElement;
		const typing = target?.matches('input,textarea,select,[contenteditable="true"]');
		if ((e.key === '/' && !typing) || ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k')) {
			e.preventDefault();
			inputEl?.focus();
			inputEl?.select();
		}
	}
	onDestroy(() => {
		++generation;
		clearTimeout(timer);
		clearTimeout(blurTimer);
	});
</script>

<svelte:window onkeydown={globalKey} />
<div class="search">
	<span class="search-icon"><Icon name="search" size={22} /></span>
	<input
		bind:this={inputEl}
		bind:value
		role="combobox"
		aria-label="Search for a word"
		aria-autocomplete="list"
		aria-expanded={open}
		aria-controls={id}
		aria-activedescendant={open && highlighted >= 0 ? `${id}-${highlighted}` : undefined}
		placeholder="Search a word…"
		autocomplete="off"
		spellcheck="false"
		oninput={schedule}
		onkeydown={key}
		onfocus={() => {
			clearTimeout(blurTimer);
			schedule();
		}}
		onblur={() => {
			blurTimer = setTimeout(close, 120);
		}}
		oncompositionstart={() => {
			composing = true;
			close();
		}}
		oncompositionend={() => {
			composing = false;
			schedule();
		}}
	/>
	<div class="search-actions">
		{#if value}<button
				class="icon-button ghost"
				aria-label="Clear search"
				onclick={() => {
					value = '';
					schedule();
					inputEl.focus();
				}}><Icon name="close" size={16} /></button
			>{:else}<kbd>Ctrl K</kbd>{/if}<button
			class="primary submit"
			aria-label="Look up"
			onclick={() => commit(value)}><Icon name="arrow" size={20} /></button
		>
	</div>
	{#if open}<ul class="suggestions" {id} role="listbox" aria-label="Suggestions">
			{#each suggestions as s, i (s.word)}<li
					id={`${id}-${i}`}
					role="option"
					aria-selected={i === highlighted}
					class:selected={i === highlighted}
					onpointerdown={(e) => {
						e.preventDefault();
						commit(s.word);
					}}
				>
					<span><strong>{s.display}</strong><small>{s.phonetic ?? ''}</small></span><Icon
						name="arrow"
						size={16}
					/>
				</li>{/each}
			<li class="search-help" role="presentation">↑ ↓ select · Enter look up · Esc close</li>
		</ul>{/if}
</div>
{#if searchError}<p class="small muted" role="status">{searchError}</p>{/if}

<style>
	.search {
		position: relative;
	}
	.search input {
		width: 100%;
		height: 62px;
		padding: 16px 116px 16px 52px;
		font-size: 16px;
		letter-spacing: -0.012em;
		border-radius: 12px;
		box-shadow: var(--shadow);
	}
	.search-icon {
		position: absolute;
		left: 20px;
		top: 21px;
		color: var(--muted);
		pointer-events: none;
	}
	.search-actions {
		position: absolute;
		right: 11px;
		top: 12px;
		display: flex;
		align-items: center;
		gap: 12px;
	}
	.submit {
		width: 38px;
		height: 38px;
		padding: 10px;
	}
	.suggestions {
		position: absolute;
		z-index: 20;
		left: 0;
		right: 0;
		top: calc(100% + 6px);
		margin: 0;
		padding: 6px;
		list-style: none;
		background: var(--card);
		border: 1px solid var(--line);
		border-radius: 10px;
		box-shadow: 0 8px 24px #25252514;
		animation: drop 0.16s var(--ease) both;
	}
	@keyframes drop {
		from {
			opacity: 0;
			transform: translateY(-5px);
		}
	}
	.suggestions li {
		padding: 9px 13px;
		border-radius: 6px;
		display: flex;
		align-items: center;
		justify-content: space-between;
		cursor: pointer;
		transition: background 0.12s var(--ease);
	}
	.suggestions li.selected,
	.suggestions li:hover {
		background: var(--soft);
	}
	.suggestions strong {
		font: 500 19px var(--serif);
		margin-right: 18px;
	}
	.suggestions small {
		color: var(--muted);
		font-size: 12px;
	}
	.suggestions li.search-help {
		font-size: 10px;
		color: var(--muted);
		border-top: 1px solid var(--line);
		margin-top: 5px;
		border-radius: 0;
		cursor: default;
	}
	@media (max-width: 520px) {
		.search input {
			font-size: 14px;
			padding-right: 100px;
		}
		.search-actions {
			gap: 4px;
		}
		.search-actions kbd {
			display: none;
		}
	}
</style>
