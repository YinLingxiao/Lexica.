<script lang="ts">
	import { preferences } from '$lib/preferences.svelte';
	import { dictionaryImport } from '$lib/import-state.svelte';
	import { appInfo, importEcdict, errorMessage, isPreview, type AppInfo } from '$lib/api';
	import Icon from '$lib/components/Icon.svelte';
	let info = $state<AppInfo | null>(null);
	let error = $state('');
	async function refresh() {
		error = '';
		try {
			info = await appInfo();
		} catch (e) {
			error = errorMessage(e);
		}
	}
	async function doImport() {
		if (!dictionaryImport.sourcePath.trim() || dictionaryImport.running) return;
		dictionaryImport.running = true;
		dictionaryImport.message = '';
		dictionaryImport.error = '';
		try {
			const count = await importEcdict(dictionaryImport.sourcePath.trim());
			dictionaryImport.message = `Imported ${count.toLocaleString()} new entries.`;
			await refresh();
		} catch (e) {
			dictionaryImport.error = errorMessage(e);
		} finally {
			dictionaryImport.running = false;
		}
	}
	$effect(() => {
		if (!dictionaryImport.running) void refresh();
	});
</script>

<main class="page settings">
	<h1>Settings</h1>
	<section class="panel">
		<div class="setting-row">
			<h3>Theme</h3>
			<div class="segmented">
				{#each [['light', 'Light'], ['dark', 'Dark'], ['system', 'System']] as [key, label]}<button
						aria-pressed={preferences.theme === key}
						class:chosen={preferences.theme === key}
						onclick={() => (preferences.theme = key as 'light' | 'dark' | 'system')}>{label}</button
					>{/each}
			</div>
		</div>
		<div class="setting-row">
			<h3>Lookup</h3>
			<div class="segmented">
				{#each [['guided', 'Context first'], ['full', 'Full entry']] as [key, label]}<button
						aria-pressed={preferences.reading === key}
						class:chosen={preferences.reading === key}
						onclick={() => (preferences.reading = key)}>{label}</button
					>{/each}
			</div>
		</div>
		<div class="setting-row last">
			<h3>Text size</h3>
			<div class="segmented">
				{#each [['normal', 'Normal'], ['large', 'Large']] as [key, label]}<button
						aria-pressed={preferences.fontSize === key}
						class:chosen={preferences.fontSize === key}
						onclick={() => (preferences.fontSize = key)}>{label}</button
					>{/each}
			</div>
		</div>
	</section>
	<section class="panel dictionary">
		<div class="section-heading"><h2>Dictionary</h2></div>
		{#if error}<p class="error" role="alert">{error}</p>
			<button onclick={refresh}>Retry</button>
		{:else if info}<div class="dictionary-info">
				<div><strong>{info.word_count.toLocaleString()}</strong><span>entries</span></div>
				<span class="badge"
					>{isPreview() ? 'Preview · read-only' : info.fts_ok ? 'Index ready' : 'Index error'}</span
				>
			</div>
		{:else}<p class="muted small">Loading…</p>{/if}
		<label for="source-path">Import ECDICT (stardict.db)</label>
		<form
			class="import-row"
			onsubmit={(e) => {
				e.preventDefault();
				void doImport();
			}}
		>
			<input
				id="source-path"
				bind:value={dictionaryImport.sourcePath}
				placeholder="D:\Dictionary\stardict.db"
				disabled={dictionaryImport.running || isPreview()}
				spellcheck="false"
			/><button
				class="primary"
				disabled={!dictionaryImport.sourcePath.trim() || dictionaryImport.running || isPreview()}
				><Icon name="download" size={15} />{dictionaryImport.running
					? 'Importing…'
					: 'Import'}</button
			>
		</form>
		{#if dictionaryImport.running}<p class="import-note" role="status">
				Writing entries — keep the app open. This can take a few minutes.
			</p>{/if}{#if dictionaryImport.message}<p class="import-note" role="status">
				{dictionaryImport.message}
			</p>{/if}{#if dictionaryImport.error}<p class="error" role="alert">
				{dictionaryImport.error}
			</p>{/if}
	</section>
	<section class="panel shortcuts">
		<div class="section-heading"><h2>Shortcuts</h2></div>
		<dl>
			{#each [['Focus search', ['Ctrl', 'K']], ['Suggestions', ['↑', '↓', 'Enter']], ['Understood / English / Full entry', ['U', '2', '3']], ['Submit answer, next item', ['Enter']]] as [label, keys]}<div
				>
					<dt>{label}</dt>
					<dd>
						{#each keys as k}<kbd>{k}</kbd>{/each}
					</dd>
				</div>{/each}
		</dl>
	</section>
	<p class="about">
		Dictionary, bookmarks, notes and learning records stay on this device. Pronunciation uses the
		system offline English voice.
	</p>
</main>

<style>
	.settings {
		max-width: 780px;
	}
	.settings h1 {
		font-size: 24px;
		margin-bottom: 22px;
	}
	.settings > .panel {
		margin-bottom: 20px;
	}
	.setting-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 20px;
		padding: 16px 0;
		border-bottom: 1px solid var(--line);
	}
	.setting-row:first-child {
		padding-top: 0;
	}
	.setting-row.last {
		border: 0;
		padding-bottom: 0;
	}
	.setting-row h3 {
		font-size: 13px;
		margin: 0;
		font-weight: 500;
	}
	.segmented {
		display: flex;
		gap: 4px;
		padding: 4px;
		background: var(--bg);
		border: 1px solid var(--line);
		border-radius: 10px;
	}
	.segmented button {
		white-space: nowrap;
		padding: 6px 12px;
		border: 0;
		background: transparent;
		color: var(--muted);
		font-size: 11px;
	}
	.segmented button.chosen {
		background: var(--card);
		color: var(--accent);
	}
	.dictionary-info {
		display: flex;
		align-items: center;
		justify-content: space-between;
		background: var(--soft);
		border-radius: 10px;
		padding: 16px 18px;
		margin-bottom: 22px;
	}
	.dictionary-info strong {
		font: 500 26px var(--serif);
		margin-right: 9px;
	}
	.dictionary-info > div span {
		font-size: 11px;
		color: var(--muted);
	}
	.dictionary label {
		display: block;
		font-size: 11px;
		color: var(--muted);
		margin-bottom: 8px;
	}
	.import-row {
		display: flex;
		gap: 10px;
	}
	.import-row input {
		flex: 1;
		min-width: 0;
		font-size: 12px;
	}
	.import-row button {
		font-size: 12px;
		white-space: nowrap;
	}
	.import-note {
		color: var(--accent);
		font-size: 11px;
		margin: 12px 0 0;
	}
	.dictionary .error {
		margin-top: 14px;
	}
	.shortcuts dl {
		margin: 0;
	}
	.shortcuts dl > div {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 20px;
		padding: 11px 0;
		border-bottom: 1px solid var(--line);
		font-size: 12px;
	}
	.shortcuts dl > div:last-child {
		border: 0;
		padding-bottom: 0;
	}
	.shortcuts dt {
		color: var(--muted);
	}
	.shortcuts dd {
		display: flex;
		gap: 5px;
		margin: 0;
		white-space: nowrap;
	}
	.about {
		font-size: 11px;
		color: var(--muted);
		text-align: center;
		line-height: 2;
		margin: 28px auto 0;
		max-width: 460px;
	}
	@media (max-width: 600px) {
		.setting-row {
			align-items: flex-start;
			flex-direction: column;
			gap: 12px;
		}
		.segmented {
			width: 100%;
		}
		.segmented button {
			flex: 1;
		}
		.import-row {
			flex-direction: column;
		}
		.dictionary-info {
			align-items: flex-start;
			flex-direction: column;
			gap: 10px;
		}
	}
</style>
