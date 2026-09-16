<script lang="ts">
	import { onMount } from 'svelte';
	import { openUrl } from '@tauri-apps/plugin-opener';
	import { preferences } from '$lib/preferences.svelte';
	import { dictionaryImport } from '$lib/import-state.svelte';
	import { dictionarySetup, runDictionarySetup } from '$lib/dictionary-setup.svelte';
	import {
		appInfo,
		importEcdict,
		aiConfigGet,
		aiConfigSave,
		aiTestConnection,
		errorMessage,
		isPreview,
		type AppInfo
	} from '$lib/api';
	import Icon from '$lib/components/Icon.svelte';
	let info = $state<AppInfo | null>(null);
	let error = $state('');
	let logoError = $state('');

	async function openGithub(event: MouseEvent) {
		if (isPreview()) return;
		event.preventDefault();
		logoError = '';
		try {
			await openUrl('https://github.com/YinLingxiao');
		} catch (cause) {
			logoError = `Could not open GitHub: ${errorMessage(cause)}`;
		}
	}

	// AI 例句（可选，默认关闭）。密钥只在 Rust 端的 Windows 凭据管理器里；
	// 前端只持有输入框里的临时值，保存后立即清空。
	let ai = $state({
		enabled: false,
		baseUrl: 'https://api.deepseek.com/v1',
		model: 'deepseek-chat',
		key: '',
		hasKey: false
	});
	let aiSaving = $state(false);
	let aiTesting = $state(false);
	let aiMessage = $state('');
	let aiError = $state('');

	async function loadAi() {
		try {
			const cfg = await aiConfigGet();
			ai.enabled = cfg.enabled;
			ai.baseUrl = cfg.base_url;
			ai.model = cfg.model;
			ai.hasKey = cfg.has_key;
			ai.key = '';
		} catch {
			/* AI 配置读取失败不打扰其他设置 */
		}
	}

	async function saveAi(keyAction: 'keep' | 'set' | 'delete') {
		if (aiSaving || isPreview()) return;
		aiSaving = true;
		aiMessage = '';
		aiError = '';
		try {
			const key = keyAction === 'set' ? ai.key : keyAction === 'delete' ? '' : null;
			const cfg = await aiConfigSave(ai.enabled, ai.baseUrl, ai.model, key);
			ai.enabled = cfg.enabled;
			ai.baseUrl = cfg.base_url;
			ai.model = cfg.model;
			ai.hasKey = cfg.has_key;
			ai.key = '';
			aiMessage = 'AI settings saved.';
		} catch (e) {
			aiError = errorMessage(e);
		} finally {
			aiSaving = false;
		}
	}

	async function testAi() {
		if (aiTesting || isPreview()) return;
		aiTesting = true;
		aiMessage = '';
		aiError = '';
		try {
			await aiTestConnection();
			aiMessage = 'Connection OK.';
		} catch (e) {
			aiError = errorMessage(e);
		} finally {
			aiTesting = false;
		}
	}

	async function refresh() {
		error = '';
		try {
			info = await appInfo();
		} catch (e) {
			error = errorMessage(e);
		}
	}
	onMount(() => {
		void loadAi();
	});
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
	async function doDownload() {
		if (dictionarySetup.active || isPreview()) return;
		try {
			await runDictionarySetup(true);
			await refresh();
		} catch {
			/* overlay shows the error */
		}
	}
	$effect(() => {
		if (!dictionaryImport.running && !dictionarySetup.active) void refresh();
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
					>{isPreview()
						? 'Preview · read-only'
						: info.dictionary_ready
							? 'ECDICT ready'
							: info.fts_ok
								? 'Seed only'
								: 'Index error'}</span
				>
			</div>
		{:else}<p class="muted small">Loading…</p>{/if}
		<p class="muted small dict-blurb">
			Free ECDICT corpus (~3.4M entries, MIT). Downloaded to the app data folder on first launch.
			<a
				href="https://github.com/skywind3000/ECDICT/releases/tag/1.0.28"
				target="_blank"
				rel="noreferrer">Download page ↗</a
			>
		</p>
		<button
			class="primary download-btn"
			disabled={dictionarySetup.active || isPreview()}
			onclick={doDownload}
			><Icon name="download" size={15} />{info?.dictionary_ready
				? 'Re-check dictionary'
				: 'Download dictionary'}</button
		>
		<details class="advanced">
			<summary>Import local stardict.db</summary>
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
					>{dictionaryImport.running ? 'Importing…' : 'Import'}</button
				>
			</form>
			{#if dictionaryImport.running}<p class="import-note" role="status">
					Writing entries — keep the app open.
				</p>{/if}{#if dictionaryImport.message}<p class="import-note" role="status">
					{dictionaryImport.message}
				</p>{/if}{#if dictionaryImport.error}<p class="error" role="alert">
					{dictionaryImport.error}
				</p>{/if}
		</details>
	</section>
	<section class="panel ai" id="ai-examples">
		<div class="section-heading"><h2>DeepSeek example sentences</h2></div>
		<p class="muted small ai-blurb">
			DeepSeek is preconfigured and remains off until you add an API key and enable it. When you look
			up a word with no dictionary example, it generates a short sentence immediately. In review,
			AI supplies the fill-in-the-blank sentences. Only the word and one
			definition are sent — notes and learning history stay local. The key is stored in the Windows
			credential manager, never in the database.
		</p>
		<div class="setting-row">
			<h3>Enable</h3>
			<div class="segmented">
				<button
					aria-pressed={!ai.enabled}
					class:chosen={!ai.enabled}
					onclick={() => (ai.enabled = false)}>Off</button
				><button
					aria-pressed={ai.enabled}
					class:chosen={ai.enabled}
					onclick={() => (ai.enabled = true)}>On</button
				>
			</div>
		</div>
		{#if ai.enabled}
			<div class="ai-fields">
				<label>
					<span>Service address (API base URL)</span>
					<input
						bind:value={ai.baseUrl}
						placeholder="https://api.deepseek.com/v1"
						spellcheck="false"
						disabled={aiSaving}
					/>
				</label>
				<label>
					<span>Model</span>
					<input
						bind:value={ai.model}
						placeholder="deepseek-chat"
						spellcheck="false"
						disabled={aiSaving}
					/>
				</label>
				<label>
					<span>API key{#if ai.hasKey}<em class="muted"> · stored</em>{/if}</span>
					<input
						type="password"
						bind:value={ai.key}
						placeholder={ai.hasKey ? 'Leave empty to keep the stored key' : 'Paste API key'}
						spellcheck="false"
						autocomplete="off"
						disabled={aiSaving}
					/>
				</label>
				<div class="ai-actions">
					<button disabled={aiSaving} onclick={() => saveAi(ai.key.trim() ? 'set' : 'keep')}
						>{aiSaving ? 'Saving…' : 'Save'}</button
					><button class="ghost" disabled={aiTesting || aiSaving} onclick={testAi}
						>{aiTesting ? 'Testing…' : 'Test connection'}</button
					>{#if ai.hasKey}<button
							class="ghost"
							disabled={aiSaving}
							onclick={() => saveAi('delete')}>Remove key</button
						>{/if}
				</div>
				{#if aiMessage}<p class="import-note" role="status">{aiMessage}</p>{/if}
				{#if aiError}<p class="error" role="alert">{aiError}</p>{/if}
			</div>
		{/if}
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
	<div class="credit">
		<a
			href="https://github.com/YinLingxiao"
			target="_blank"
			rel="noopener noreferrer"
			class="credit-logo"
			aria-label="Visit Yin Lingxiao on GitHub"
			aria-describedby="credit-tip"
			onclick={openGithub}
		>
			<span class="credit-mark" aria-hidden="true"></span>
			<span class="tooltip" id="credit-tip" role="tooltip">Designed by Y.I.A.</span>
		</a>
	</div>
	{#if logoError}<p class="logo-error" role="alert">{logoError}</p>{/if}
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
	.dict-blurb {
		margin: 0 0 16px;
		line-height: 1.7;
	}
	.download-btn {
		width: 100%;
		margin-bottom: 18px;
	}
	.advanced {
		font-size: 12px;
		color: var(--muted);
	}
	.advanced summary {
		cursor: pointer;
		margin-bottom: 10px;
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
	.ai-blurb {
		line-height: 1.7;
		margin: 0 0 6px;
	}
	.ai-fields {
		display: flex;
		flex-direction: column;
		gap: 14px;
		padding-top: 14px;
	}
	.ai-fields label {
		display: flex;
		flex-direction: column;
		gap: 6px;
		font-size: 12px;
	}
	.ai-fields label span {
		color: var(--muted);
	}
	.ai-fields input {
		font-size: 13px;
	}
	.ai-fields em {
		font-style: normal;
		font-size: 11px;
	}
	.ai-actions {
		display: flex;
		gap: 8px;
		flex-wrap: wrap;
	}
	.ai-actions button {
		font-size: 12px;
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
	.credit {
		display: flex;
		justify-content: center;
		margin-top: 48px;
		padding-bottom: 12px;
	}
	.credit-logo {
		position: relative;
		display: inline-block;
		border: 0;
		border-radius: 12px;
		background: none;
		padding: 0;
		cursor: pointer;
		text-decoration: none;
	}
	.credit-logo:focus-visible {
		outline: 2px solid var(--fg);
		outline-offset: 6px;
	}
	.credit-mark {
		display: block;
		width: min(160px, 50vw);
		aspect-ratio: 1206 / 1304;
		background: var(--fg);
		-webkit-mask: url('/mogian-logo-cutout.png') center / contain no-repeat;
		mask: url('/mogian-logo-cutout.png') center / contain no-repeat;
	}
	.credit-logo .tooltip {
		position: absolute;
		bottom: calc(100% + 10px);
		left: 50%;
		transform: translate(-50%, 4px);
		opacity: 0;
		pointer-events: none;
		background: var(--fg);
		color: var(--bg);
		font-size: 11px;
		white-space: nowrap;
		padding: 6px 10px;
		border-radius: 6px;
		transition:
			opacity 0.16s var(--ease),
			transform 0.16s var(--ease);
	}
	.credit-logo .tooltip::after {
		content: '';
		position: absolute;
		top: 100%;
		left: 50%;
		transform: translateX(-50%);
		border: 5px solid transparent;
		border-top-color: var(--fg);
	}
	.credit-logo:hover .tooltip,
	.credit-logo:focus-visible .tooltip {
		opacity: 1;
		transform: translate(-50%, 0);
	}
	.logo-error {
		color: var(--danger);
		font-size: 12px;
		text-align: center;
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
