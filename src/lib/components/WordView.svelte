<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { beforeNavigate } from '$app/navigation';
	import Icon from './Icon.svelte';
	import HighlightedText from './HighlightedText.svelte';
	import AiExampleLoading from './AiExampleLoading.svelte';
	import { preferences } from '$lib/preferences.svelte';
	import {
		setComprehension,
		setWordIgnored,
		wordNote,
		saveWordNote,
		errorMessage,
		statusLabel,
		isPreview,
		aiConfigGet,
		aiGenerateExamples,
		type ComprehensionLevel,
		type LookupWordResult
	} from '$lib/api';
	let {
		result,
		query,
		oncommit,
		ondismiss,
		onupdate
	}: {
		result: LookupWordResult;
		query: string;
		oncommit: (word: string) => void;
		ondismiss: () => void;
		onupdate: () => void;
	} = $props();
	const entry = $derived(result.outcome.type === 'hit' ? result.outcome.entry : null);
	const miss = $derived(result.outcome.type === 'miss' ? result.outcome : null);
	let aiSentence = $state('');
	let aiLoading = $state(false);
	let aiUnavailable = $state(false);
	const dictionaryExamples = $derived(
		entry?.senses.flatMap((s) => s.examples.filter((example) => example.text.trim())) ?? []
	);
	const examples = $derived(
		aiSentence ? [{ text: aiSentence, translation: null }] : dictionaryExamples
	);

	const primary = $derived(entry?.senses[0]);
	let full = $state(false);
	let showEnglish = $state(false);
	let exampleIdx = $state(0);
	const example = $derived(examples[exampleIdx % Math.max(1, examples.length)]);
	let bookmarked = $state(false);
	let ignored = $state(false);
	let note = $state('');
	let savedNote = $state('');
	let noteReady = $state(false);
	let noteError = $state('');
	let busy = $state(false);
	let message = $state('');
	let error = $state('');
	let voices = $state<SpeechSynthesisVoice[]>([]);
	const dirty = $derived(note !== savedNote);
	let mounted = true;
	let recordedRank = 0;
	let recordChain = Promise.resolve();
	async function loadAiExample() {
		if (!entry || dictionaryExamples.length || !entry.senses.length || isPreview()) return;
		try {
			const config = await aiConfigGet();
			if (!mounted || !config.enabled) return;
			aiLoading = true;
			const generated = await aiGenerateExamples([{ word_id: entry.id, sense_id: null }]);
			if (!mounted) return;
			const sentence = generated.find((item) => item.word_id === entry.id)?.sentence;
			if (sentence?.includes('______')) {
				aiSentence = sentence.replaceAll('______', entry.word).replace(/^./, (letter) =>
					letter.toUpperCase()
				);
			} else {
				aiUnavailable = true;
			}
		} catch {
			if (mounted) aiUnavailable = true;
		} finally {
			if (mounted) aiLoading = false;
		}
	}
	const ranks: Record<ComprehensionLevel, number> = {
		unknown: 0,
		context: 1,
		english: 2,
		chinese: 3
	};
	function record(level: ComprehensionLevel) {
		if (!result.encounter_id || ranks[level] <= recordedRank) return;
		const id = result.encounter_id;
		recordChain = recordChain.then(async () => {
			if (ranks[level] <= recordedRank) return;
			try {
				await setComprehension(id, level);
				recordedRank = ranks[level];
				onupdate();
			} catch (e) {
				if (mounted) error = `Learning record could not be saved: ${errorMessage(e)}`;
			}
		});
	}
	function revealEnglish() {
		showEnglish = true;
		record('english');
	}
	function revealFull() {
		full = true;
		record(entry?.senses.some((s) => s.chinese_definition) ? 'chinese' : 'english');
	}
	function understand() {
		full = true;
		record(showEnglish || !example ? 'english' : 'context');
	}
	async function loadNote() {
		if (!entry) return;
		noteError = '';
		noteReady = false;
		try {
			const data = await wordNote(entry.id);
			if (!mounted) return;
			bookmarked = data.bookmarked;
			ignored = data.ignored;
			note = data.note;
			savedNote = data.note;
			noteReady = true;
		} catch (e) {
			noteError = errorMessage(e);
		}
	}
	async function save(bookmark = bookmarked) {
		if (!entry || !noteReady || busy) return;
		busy = true;
		error = '';
		message = '';
		const snapshot = note;
		try {
			await saveWordNote(entry.id, bookmark, snapshot);
			bookmarked = bookmark;
			savedNote = snapshot;
			message = bookmark ? 'Bookmarked' : 'Saved';
			onupdate();
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = false;
		}
	}
	async function toggleTracking() {
		if (!entry || busy) return;
		busy = true;
		error = '';
		try {
			await setWordIgnored(entry.id, !ignored);
			ignored = !ignored;
			message = ignored ? 'Review reminders paused' : 'Review reminders resumed';
			onupdate();
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = false;
		}
	}
	async function copy() {
		if (!entry) return;
		try {
			await navigator.clipboard.writeText(
				`${entry.display} ${entry.phonetic ?? ''}\n${entry.senses.map((s) => [s.english_definition, s.chinese_definition].filter(Boolean).join('\n')).join('\n\n')}`
			);
			message = 'Entry copied';
		} catch {
			error = 'Clipboard unavailable — select the text and copy it manually.';
		}
	}
	function syncVoices() {
		if ('speechSynthesis' in window)
			voices = window.speechSynthesis
				.getVoices()
				.filter((v) => v.localService && v.lang.toLowerCase().startsWith('en'));
	}
	function speak() {
		if (!entry || !voices.length) return;
		speechSynthesis.cancel();
		const utterance = new SpeechSynthesisUtterance(entry.word);
		utterance.voice = voices[0];
		utterance.lang = voices[0].lang;
		utterance.rate = 0.85;
		utterance.onerror = () => {
			if (mounted) error = 'Pronunciation unavailable — check the system English voice pack.';
		};
		speechSynthesis.speak(utterance);
	}
	function keys(e: KeyboardEvent) {
		if (
			e.isComposing ||
			e.ctrlKey ||
			e.metaKey ||
			e.altKey ||
			(e.target as HTMLElement)?.matches('input,textarea,select,button,a,[contenteditable="true"]')
		)
			return;
		if (e.key === 'Escape') ondismiss();
		if (!full && e.key.toLowerCase() === 'u') understand();
		if (!full && e.key === '2') revealEnglish();
		if (!full && e.key === '3') revealFull();
	}
	beforeNavigate((navigation) => {
		if (dirty && !confirm('You have unsaved notes. Leave anyway?')) navigation.cancel();
	});
	onMount(() => {
		recordedRank = ranks[result.comprehension_level ?? 'unknown'];
		full = preferences.reading === 'full';
		if (full) record(entry?.senses.some((s) => s.chinese_definition) ? 'chinese' : 'english');
		void loadNote();
		void loadAiExample();
		syncVoices();
		window.speechSynthesis?.addEventListener('voiceschanged', syncVoices);
	});
	onDestroy(() => {
		mounted = false;
		if (typeof window !== 'undefined') {
			window.speechSynthesis?.removeEventListener('voiceschanged', syncVoices);
			window.speechSynthesis?.cancel();
		}
	});
</script>

<svelte:window
	onkeydown={keys}
	onbeforeunload={(e) => {
		if (dirty) {
			e.preventDefault();
			e.returnValue = '';
		}
	}}
/>
{#if miss}
	<section class="panel empty rise">
		<div class="empty-icon"><Icon name="search" size={28} /></div>
		<h2>No entry for “{query}”</h2>
		<p>Check the spelling, or try the base form of the word.</p>
		{#if miss.suggestions.length}<div class="related">
				{#each miss.suggestions as s}<button onclick={() => oncommit(s.word)}
						>{s.display}<Icon name="arrow" size={14} /></button
					>{/each}
			</div>{/if}
	</section>
{:else if entry}
	<article class="word-layout rise">
		<div class="main-column">
			<section class="panel word-card">
				<div class="word-top">
					<div>
						<h1>{entry.display}</h1>
						<div class="phonetic">
							{#if entry.phonetic}{entry.phonetic}{/if}<button
								class="icon-button ghost"
								disabled={!voices.length}
								onclick={speak}
								aria-label="Play pronunciation"
								title={voices.length
									? 'Play pronunciation (offline voice)'
									: 'No offline English voice installed'}><Icon name="sound" size={18} /></button
							>
						</div>
					</div>
					<div class="word-tools">
						<button
							class="icon-button"
							aria-label={bookmarked ? 'Remove bookmark' : 'Bookmark this word'}
							title={bookmarked ? 'Remove bookmark' : 'Bookmark this word'}
							aria-pressed={bookmarked}
							class:saved={bookmarked}
							onclick={() => save(!bookmarked)}
							disabled={!noteReady || busy || isPreview()}
							><Icon name="bookmark" size={18} /></button
						><button class="icon-button" aria-label="Copy entry" title="Copy entry" onclick={copy}
							><Icon name="copy" size={17} /></button
						>
					</div>
				</div>
				<div class="word-meta">
					{#if primary?.pos}<span class="badge">{primary.pos}</span>{/if}{#if primary?.level}<span
							class="badge level">{primary.level}</span
						>{/if}{#if result.memory}<span class="muted small"
							>{statusLabel[result.memory.status ?? 'new']} · seen {result.memory
								.visit_count}×</span
						>{/if}{#if ignored}<span class="badge level">Paused</span>{/if}
				</div>
				{#if !full}
					<div class="context">
						{#if example}<p class="reading example">
								<HighlightedText text={example.text} word={entry.word} />
							</p>
							{#if aiSentence}<span class="ai-source">AI example</span>{/if}
							{#if examples.length > 1}<button class="ghost another" onclick={() => exampleIdx++}
									>Another example <span
										>{(exampleIdx % examples.length) + 1} / {examples.length}</span
									><Icon name="arrow" size={14} /></button
								>{/if}{:else if aiLoading}<AiExampleLoading />{:else}<p class="muted">
								{aiUnavailable
									? 'AI example unavailable — see the definitions.'
									: 'No example sentence for this word — see the definitions.'}
							</p>{/if}
					</div>
					{#if showEnglish}<div class="english-hint">
							<p class="reading">
								{primary?.english_definition || 'No English definition — see the full entry.'}
							</p>
						</div>{/if}
					<div class="disclosure-actions">
						<button class="primary" onclick={understand} disabled={!example && !showEnglish}
							>Understood <kbd>U</kbd></button
						>{#if !showEnglish}<button onclick={revealEnglish}>English <kbd>2</kbd></button
							>{/if}<button class="ghost" onclick={revealFull}
							>Full entry <Icon name="arrow" size={15} /></button
						>
					</div>
				{:else}
					<div class="definitions">
						<div class="section-heading">
							<h2>Definitions</h2>
						</div>
						{#each entry.senses as sense, i}<section class="sense">
								<div class="sense-number">{String(i + 1).padStart(2, '0')}</div>
								<div class="sense-body">
									<div class="sense-tags">
										{#if sense.pos}{sense.pos}{/if}{#if sense.level}<span class="badge"
												>{sense.level}</span
											>{/if}
									</div>
									{#if sense.english_definition}<p class="reading english">
											{sense.english_definition}
										</p>{/if}{#if sense.chinese_definition}<p class="chinese">
											{sense.chinese_definition}
										</p>{/if}{#each sense.examples as ex}<blockquote>
											<p class="reading"><HighlightedText text={ex.text} word={entry.word} /></p>
											{#if ex.translation}<footer>{ex.translation}</footer>{/if}
										</blockquote>{/each}
									{#if i === 0 && aiSentence}<blockquote>
											<p class="reading"><HighlightedText text={aiSentence} word={entry.word} /></p>
											<footer>AI example</footer>
										</blockquote>{:else if i === 0 && aiLoading}<AiExampleLoading />{/if}
								</div>
							</section>{/each}{#if !entry.senses.length}<p class="muted">
								No definitions available.
							</p>{/if}
					</div>
				{/if}
			</section>
			{#if full && entry.collocations.length}<section class="panel extra">
					<div class="section-heading">
						<h2>Collocations</h2>
					</div>
					{#each entry.collocations as c}<div class="collocation">
							<span>{c.text}</span><small>{c.gloss ?? ''}</small>
						</div>{/each}
				</section>{/if}
			{#if full}{#each [{ title: 'Synonyms', words: entry.synonyms }, { title: 'Antonyms', words: entry.antonyms }] as group}{#if group.words.length}<section
							class="panel extra"
						>
							<h2>{group.title}</h2>
							<div class="related">
								{#each group.words as w}<button onclick={() => oncommit(w)}
										>{w}<Icon name="arrow" size={13} /></button
									>{/each}
							</div>
						</section>{/if}{/each}{#if entry.word_family.length}<section class="panel extra">
						<h2>Word family</h2>
						<p class="word-family">{entry.word_family.join(' · ')}</p>
					</section>{/if}{/if}
		</div>
		<aside class="word-aside">
			<section class="panel notes">
				<h2>Note</h2>
				{#if noteError}<p class="error small">{noteError}</p>
					<button onclick={loadNote}>Reload</button>{:else}<textarea
						aria-label="Word notes"
						bind:value={note}
						maxlength="10000"
						rows="6"
						placeholder="Your own context or example…"
						disabled={!noteReady || isPreview()}></textarea>
					<div class="note-footer">
						<small>{dirty ? 'Unsaved' : 'Saved'}</small><button
							onclick={() => save()}
							disabled={!noteReady || busy || !dirty || isPreview()}
							>{busy ? 'Saving…' : 'Save'}</button
						>
					</div>{/if}
			</section>
			<button
				class="ghost track"
				disabled={!noteReady || busy || isPreview()}
				onclick={toggleTracking}>{ignored ? 'Resume reminders' : 'Pause reminders'}</button
			>
		</aside>
	</article>
	{#if message}<p class="save-message" role="status">
			<Icon name="check" size={16} />{message}
		</p>{/if}{#if error}<p class="error" role="alert">{error}</p>{/if}
{/if}

<style>
	.word-layout {
		display: grid;
		grid-template-columns: minmax(0, 1fr) 250px;
		gap: 24px;
	}
	.main-column {
		min-width: 0;
	}
	.word-card {
		padding: 30px;
	}
	.word-top {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 12px;
	}
	.word-top h1 {
		font: 450 46px/1.1 var(--serif);
		letter-spacing: -0.025em;
		overflow-wrap: anywhere;
		margin: 4px 0 8px;
	}
	.phonetic {
		display: flex;
		align-items: center;
		gap: 8px;
		color: var(--muted);
		font-size: 14px;
	}
	.word-tools {
		display: flex;
		gap: 6px;
	}
	.saved {
		color: var(--accent);
		background: var(--soft);
	}
	.saved :global(svg) {
		fill: var(--soft);
	}
	.word-meta {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 9px;
		margin: 15px 0 25px;
	}
	.level {
		background: var(--warm);
		color: var(--muted);
	}
	.context {
		background: var(--soft);
		padding: 24px;
		border-radius: 10px;
	}
	.example {
		font: 450 22px/1.75 var(--serif);
		margin: 0;
	}
	.ai-source {
		display: inline-block;
		margin-top: 8px;
		font-size: 10px;
		color: var(--muted);
	}
	.another {
		font-size: 10px;
		color: var(--accent);
		padding: 8px 0 0;
		gap: 12px;
	}
	.another span {
		color: var(--muted);
	}
	.english-hint {
		margin-top: 24px;
	}
	.english-hint p {
		font: 450 19px/1.7 var(--serif);
	}
	.disclosure-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		margin-top: 24px;
	}
	.disclosure-actions button {
		font-size: 11px;
	}
	.definitions {
		border-top: 1px solid var(--line);
		padding-top: 25px;
	}
	.definitions h2 {
		font-size: 14px;
	}
	.sense {
		display: flex;
		gap: 16px;
		margin: 25px 0;
	}
	.sense:last-child {
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
		font: 450 19px/1.7 var(--serif);
		white-space: pre-line;
		margin: 9px 0;
	}
	.chinese {
		font-size: 13px;
		color: var(--muted);
		white-space: pre-line;
		margin: 7px 0 18px;
	}
	blockquote {
		margin: 16px 0;
		border-left: 2px solid var(--line);
		padding: 3px 0 3px 16px;
	}
	blockquote p {
		font: 450 16px/1.8 var(--serif);
		margin: 0;
	}
	blockquote footer {
		font-size: 11px;
		color: var(--muted);
		margin-top: 6px;
	}
	.extra {
		margin-top: 18px;
	}
	.extra h2 {
		font-size: 14px;
	}
	.collocation {
		display: flex;
		flex-direction: column;
		padding: 9px 0;
		border-bottom: 1px solid var(--line);
	}
	.collocation:last-child {
		border: 0;
	}
	.collocation span,
	.word-family {
		font: 500 17px/1.8 var(--serif);
	}
	.collocation small {
		font-size: 11px;
		color: var(--muted);
	}
	.related {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		margin: 16px 0;
	}
	.related button {
		font: 500 15px var(--serif);
		padding: 8px 12px;
	}
	.notes {
		padding: 18px;
	}
	.notes h2 {
		font-size: 12px;
		margin-bottom: 10px;
	}
	.notes textarea {
		width: 100%;
		resize: vertical;
		min-height: 140px;
		font-size: 12px;
		line-height: 1.9;
		background: var(--bg);
	}
	.note-footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 6px;
		margin-top: 12px;
	}
	.note-footer small {
		font-size: 9px;
		color: var(--muted);
	}
	.note-footer button {
		font-size: 10px;
		padding: 7px 10px;
	}
	.track {
		width: 100%;
		margin-top: 10px;
		font-size: 11px;
		color: var(--muted);
	}
	.save-message {
		display: flex;
		align-items: center;
		gap: 8px;
		color: var(--accent);
		font-size: 12px;
		margin: 18px 0;
	}
	.word-family {
		margin-bottom: 0;
	}
	@media (max-width: 1100px) {
		.word-layout {
			grid-template-columns: minmax(0, 1fr);
		}
		.word-top h1 {
			font-size: 40px;
		}
	}
	@media (max-width: 600px) {
		.word-card {
			padding: 22px;
		}
		.word-top h1 {
			font-size: 33px;
		}
		.word-tools {
			flex-direction: column;
		}
		.context {
			padding: 18px;
		}
		.example {
			font-size: 20px;
		}
		.disclosure-actions button {
			padding: 9px;
		}
	}
</style>
