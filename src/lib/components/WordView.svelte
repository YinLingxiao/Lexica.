<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { beforeNavigate } from '$app/navigation';
	import Icon from './Icon.svelte';
	import HighlightedText from './HighlightedText.svelte';
	import AiExampleLoading from './AiExampleLoading.svelte';
	import { preferences } from '$lib/preferences.svelte';
	import { speakText } from '$lib/tts';
	import SyllableHeadword from './SyllableHeadword.svelte';
	import {
		setComprehension,
		setWordIgnored,
		wordNote,
		saveWordNote,
		customWordbookToggle,
		errorMessage,
		statusLabel,
		isPreview,
		aiConfigGet,
		aiGenerateExamples,
		type ComprehensionLevel,
		type LookupWordResult,
		type RelatedWordItem
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
	const primaryPos = $derived(
		entry?.senses.find((s) => s.pos?.trim())?.pos?.replace(/\.$/, '') ?? 'word'
	);

	// 词频刻度（18 格）与难度百分比（基于词频排名与 CEFR 等级计算）
	const FREQ_TICKS = 18;
	const freqFilled = $derived.by(() => {
		if (!entry) return 8;
		const r = entry.frequency_rank;
		if (!r || r <= 0) return entry.collins ? entry.collins * 3 : 8;
		if (r <= 1000) return 16;
		if (r <= 2500) return 14;
		if (r <= 5000) return 12;
		if (r <= 8000) return 10;
		if (r <= 12000) return 7;
		if (r <= 20000) return 5;
		return 3;
	});

	const difficultyPct = $derived.by(() => {
		const lvl = primary?.level?.toUpperCase();
		if (lvl === 'A1') return 18;
		if (lvl === 'A2') return 28;
		if (lvl === 'B1') return 42;
		if (lvl === 'B2') return 58;
		if (lvl === 'C1') return 76;
		if (lvl === 'C2' || lvl === 'R') return 88;
		const r = entry?.frequency_rank ?? 6000;
		return Math.min(92, Math.max(20, Math.round((r / 15000) * 80)));
	});

	// 词头标签胶囊（如 TOP3000 / Oxford3000 / CEFR-B1 / CET-4 / CET-6 / 考研）
	const headerBadges = $derived.by(() => {
		if (!entry) return [];
		const list: string[] = [];
		if (entry.frequency_rank && entry.frequency_rank <= 3000) list.push('TOP3000');
		else if (entry.frequency_rank && entry.frequency_rank <= 5000) list.push('TOP5000');
		if (entry.oxford || entry.tags?.includes('oxford')) list.push('Oxford3000');
		const lvl = primary?.level?.trim();
		if (lvl && lvl !== 'R') list.push(`CEFR-${lvl}`);
		const tagMap: Record<string, string> = {
			cet4: 'CET-4',
			cet6: 'CET-6',
			ky: '考研',
			toefl: 'TOEFL',
			ielts: 'IELTS',
			gre: 'GRE',
			gk: '高考'
		};
		for (const t of entry.tags ?? []) {
			const label = tagMap[t.toLowerCase()];
			if (label && !list.includes(label) && list.length < 5) list.push(label);
		}
		if (!list.length && primary?.level) list.push(`CEFR-${primary.level}`);
		return list;
	});

	// 汇总中文释义（用于 [translation] 块与 Derivatives 头部）
	const chineseSummary = $derived.by(() => {
		if (!entry) return '';
		const parts: string[] = [];
		let posPrefix = '';
		for (const sense of entry.senses) {
			const raw = sense.chinese_definition?.trim();
			if (!raw) continue;
			const m = /^([a-z]+\.)\s*(.+)$/i.exec(raw);
			if (m) {
				if (!posPrefix) posPrefix = m[1];
				parts.push(m[2].trim());
			} else {
				parts.push(raw);
			}
		}
		if (!parts.length) return '';
		const joined = parts.join('，');
		return posPrefix ? `${posPrefix} ${joined}` : joined;
	});

	const derivativesList = $derived.by((): RelatedWordItem[] => {
		if (!entry) return [];
		if (entry.derivatives?.length) return entry.derivatives;
		return entry.word_family.map((raw) => {
			const m = /^(\S+)\s+((?:n|v|vt|vi|adj|adv|prep|conj|pron)\.)\s*(.*)$/i.exec(raw.trim());
			if (m) return { word: m[1], pos: m[2], gloss: m[3].trim() };
			return { word: raw, pos: null, gloss: '' };
		});
	});

	let full = $state(false);
	let showEnglish = $state(false);
	let exampleIdx = $state(0);
	const example = $derived(examples[exampleIdx % Math.max(1, examples.length)]);
	let bookmarked = $state(false);
	let ignored = $state(false);
	let inCustomBook = $state(false);
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
	async function toggleCustomBook() {
		if (!entry || isPreview()) return;
		const next = !inCustomBook;
		inCustomBook = next;
		try {
			await customWordbookToggle(entry.id, next);
			message = next ? 'Added to custom wordbook' : 'Removed from custom wordbook';
		} catch (e) {
			inCustomBook = !next;
			error = errorMessage(e);
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
		if (!entry) return;
		if (!voices.length) {
			speakText(entry.word, {
				accent: preferences.study.accent,
				rate: preferences.study.speechRate
			});
			return;
		}
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
	function speakExample(text: string) {
		speakText(text, {
			accent: preferences.study.accent,
			rate: preferences.study.speechRate
		});
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
			<!-- 1. 顶部词头摘要大卡片（居中词头 + 音标喇叭 + 词频/难度 + 标签行 + 收藏/移除/复制 + 记忆小脑） -->
			<section class="panel hero-card">
				<header class="hero-head">
					<h1><SyllableHeadword word={entry.display} split={preferences.study.autoSplitSyllables} /></h1>
					<div class="phonetic-row">
						{#if entry.phonetic}<span class="phonetic">{entry.phonetic}</span>{/if}
						<button
							class="icon-button ghost sound-btn"
							disabled={!voices.length && isPreview()}
							onclick={speak}
							aria-label="Play pronunciation"
							title={voices.length
								? 'Play pronunciation (offline voice)'
								: 'Play pronunciation'}><Icon name="sound" size={18} /></button
						>
					</div>
				</header>

				<div class="metrics-bar">
					<div class="metric-item">
						<span class="metric-label">词频</span>
						<div class="freq-ticks" aria-label="Word frequency">
							{#each { length: FREQ_TICKS } as _, idx}
								<span class="freq-tick" class:on={idx < freqFilled}></span>
							{/each}
						</div>
					</div>
					<div class="metric-item">
						<span class="metric-label">难度</span>
						<div class="diff-track" aria-label="Word difficulty">
							<span class="diff-fill" style:width="{difficultyPct}%"></span>
						</div>
					</div>
				</div>

				<div class="tag-pills">
					{#each headerBadges as b}
						<span class="tag-outline">{b}</span>
					{/each}
					{#if ignored}<span class="tag-outline paused">Paused</span>{/if}
					<button
						type="button"
						class="tag-plus"
						class:active={inCustomBook}
						disabled={isPreview()}
						onclick={toggleCustomBook}
						title={inCustomBook ? '从自定义单词本移除' : '加入自定义单词本'}
						aria-label="加入自定义单词本">+</button
					>
				</div>

				<div class="action-bar">
					<div class="action-pills">
						<button
							type="button"
							class="action-pill"
							aria-label={bookmarked ? 'Remove bookmark' : 'Bookmark this word'}
							title={bookmarked ? 'Remove bookmark' : 'Bookmark this word'}
							aria-pressed={bookmarked}
							class:saved={bookmarked}
							onclick={() => save(!bookmarked)}
							disabled={!noteReady || busy || isPreview()}
						>
							<Icon name={bookmarked ? 'star-filled' : 'star'} size={15} />
							<span>{bookmarked ? '已收藏' : '收藏'}</span>
						</button>
						<button
							type="button"
							class="action-pill remove-pill"
							class:active={ignored}
							disabled={!noteReady || busy || isPreview()}
							onclick={toggleTracking}
						>
							<Icon name="trash" size={15} />
							<span>{ignored ? '已移除' : '移除'}</span>
						</button>
						<button
							type="button"
							class="action-pill"
							aria-label="Copy entry"
							title="Copy entry"
							onclick={copy}
						>
							<Icon name="copy" size={15} />
							<span>复制</span>
						</button>
					</div>
					<div
						class="memory-indicator"
						title={result.memory
							? `${statusLabel[result.memory.status ?? 'new']} · seen ${result.memory.visit_count}×`
							: 'New word'}
					>
						<Icon name="brain" size={20} />
						{#if result.memory}
							<small
								>{statusLabel[result.memory.status ?? 'new']} · {result.memory
									.visit_count}×</small
							>
						{/if}
					</div>
				</div>
			</section>

			<!-- 2. 释义与内嵌例句卡片（支持循序理解 Guided 与完整词条 Full 模式） -->
			<section class="panel word-card">
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
				{/if}
				<div class="definitions" class:spaced={!full}>
					<div class="definitions-head">
						<span class="pos-pill">{primaryPos}</span>
						<h2 class="defs-title">Definitions</h2>
					</div>
					{#each entry.senses as sense, i}
						{@const lvl = sense.level?.trim() || 'R'}
						<section class="sense-block">
							<div class="sense-head">
								<span class="level-pill" class:rare={lvl === 'R'}>{lvl}</span>
								<div class="sense-defs">
									{#if sense.english_definition}<p class="reading english">
											{sense.english_definition}
										</p>{/if}
								</div>
							</div>
							{#if full}
								{#each sense.examples as ex}
									<blockquote class="sense-example">
										<div class="ex-main">
											<p class="reading"><HighlightedText text={ex.text} word={entry.word} /></p>
											{#if ex.translation}<footer>{ex.translation}</footer>{/if}
										</div>
										<button
											type="button"
											class="icon-button ghost ex-sound"
											aria-label="朗读例句"
											title="朗读例句"
											onclick={() => speakExample(ex.text)}><Icon name="sound" size={16} /></button
										>
									</blockquote>
								{/each}
								{#if i === 0 && aiSentence}
									<blockquote class="sense-example">
										<div class="ex-main">
											<p class="reading">
												<HighlightedText text={aiSentence} word={entry.word} />
											</p>
											<footer>AI example</footer>
										</div>
										<button
											type="button"
											class="icon-button ghost ex-sound"
											aria-label="朗读例句"
											onclick={() => speakExample(aiSentence)}
											><Icon name="sound" size={16} /></button
										>
									</blockquote>
								{:else if i === 0 && aiLoading}
									<AiExampleLoading />
								{/if}
							{/if}
						</section>
					{/each}
					{#if !entry.senses.length}
						<p class="muted">No definitions available.</p>
					{/if}

					{#if chineseSummary}
						<div class="translation-summary">
							<span class="translation-pill"><em>translation</em></span>
							<p class="translation-text">{chineseSummary}</p>
						</div>
					{/if}
				</div>
			</section>

			<!-- 3. 纵向平铺的拓展卡片流（Inflection / Mnemonic / Root and Affix / Derivatives / Similar Words / Synonyms / Antonyms / Collocations） -->
			{#if entry.forms && entry.forms.length}
				<section class="panel extra-card">
					<h2 class="extra-title">Inflection</h2>
					<ul class="kv-list">
						{#each entry.forms as f}
							<li>
								<span class="kv-label">{f.label}</span>
								<strong class="kv-val">{f.form}</strong>
							</li>
						{/each}
					</ul>
				</section>
			{/if}

			{#if entry.mnemonic}
				<section class="panel extra-card">
					<h2 class="extra-title">Mnemonic</h2>
					<p class="mnemonic-body">{entry.mnemonic}</p>
				</section>
			{/if}

			{#if entry.roots && entry.roots.length}
				<section class="panel extra-card">
					<h2 class="extra-title">Root and Affix</h2>
					<ul class="kv-list">
						{#each entry.roots as r}
							<li>
								<strong class="root-key">{r.part}</strong>
								<span class="root-val">{r.meaning}</span>
							</li>
						{/each}
					</ul>
				</section>
			{/if}

			{#if derivativesList.length}
				<section class="panel extra-card">
					<h2 class="extra-title">Derivatives</h2>
					<div class="deriv-root">
						<strong>{entry.display}</strong>
					</div>
					<ul class="deriv-tree">
						{#each derivativesList as d}
							<li>
								<span class="tree-node" aria-hidden="true"></span>
								<button type="button" class="word-jump" onclick={() => oncommit(d.word)}
									>{d.word}</button
								>
								<span class="deriv-gloss">
									{#if d.pos}<em>{d.pos}</em> {/if}{d.gloss}
								</span>
							</li>
						{/each}
					</ul>
				</section>
			{/if}

			{#if entry.lookalikes && entry.lookalikes.length}
				<section class="panel extra-card">
					<h2 class="extra-title">Similar Words</h2>
					<ul class="kv-list">
						{#each entry.lookalikes as item}
							<li>
								<button type="button" class="word-jump" onclick={() => oncommit(item.word)}
									>{item.word}</button
								>
								<span class="deriv-gloss">
									{#if item.pos}<em>{item.pos}</em> {/if}{item.gloss}
								</span>
							</li>
						{/each}
					</ul>
				</section>
			{/if}

			{#each [{ title: 'Synonyms', words: entry.synonyms }, { title: 'Antonyms', words: entry.antonyms }] as group}
				{#if group.words.length}
					<section class="panel extra-card">
						<h2 class="extra-title">{group.title}</h2>
						<div class="related-cloud">
							{#each group.words as w}
								<button type="button" class="cloud-word" onclick={() => oncommit(w)}>
									{w}
								</button>
							{/each}
						</div>
					</section>
				{/if}
			{/each}

			{#if entry.collocations.length}
				<section class="panel extra-card">
					<h2 class="extra-title">Collocations</h2>
					{#each entry.collocations as c}
						<div class="collocation">
							<span>{c.text}</span><small>{c.gloss ?? ''}</small>
						</div>
					{/each}
				</section>
			{/if}
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
						disabled={!noteReady || isPreview()}
					></textarea>
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
		gap: 20px;
	}
	.main-column {
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 16px;
	}
	.main-column > .panel {
		animation: card-in 520ms var(--ease) both;
		animation-delay: 360ms;
	}
	.main-column > .panel:nth-child(1) {
		animation-delay: 0ms;
	}
	.main-column > .panel:nth-child(2) {
		animation-delay: 90ms;
	}
	.main-column > .panel:nth-child(3) {
		animation-delay: 170ms;
	}
	.main-column > .panel:nth-child(4) {
		animation-delay: 240ms;
	}
	.main-column > .panel:nth-child(5) {
		animation-delay: 300ms;
	}
	@keyframes card-in {
		from {
			opacity: 0;
			transform: translateY(14px) scale(0.99);
		}
	}

	/* ── 1. 顶部词头摘要卡片 ─────────────────────────────────── */
	.hero-card {
		padding: 28px 30px 22px;
	}
	.hero-head {
		text-align: center;
		margin-bottom: 20px;
	}
	.hero-head h1 {
		font: 600 52px/1.08 var(--serif);
		letter-spacing: -0.028em;
		overflow-wrap: anywhere;
		margin: 0 0 10px;
		color: var(--fg);
	}
	.phonetic-row {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: 6px;
		color: var(--muted);
		font-size: 16px;
	}
	.sound-btn {
		color: var(--accent);
	}

	.metrics-bar {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 28px;
		flex-wrap: wrap;
		margin-bottom: 18px;
	}
	.metric-item {
		display: inline-flex;
		align-items: center;
		gap: 10px;
	}
	.metric-label {
		font-size: 12px;
		color: var(--muted);
	}
	.freq-ticks {
		display: inline-flex;
		align-items: center;
		gap: 3px;
	}
	.freq-tick {
		width: 2.5px;
		height: 11px;
		border-radius: 1px;
		background: var(--line);
	}
	.freq-tick.on {
		background: var(--accent);
	}
	.diff-track {
		width: 110px;
		height: 8px;
		border-radius: 999px;
		background: var(--soft);
		border: 1px solid var(--line);
		overflow: hidden;
	}
	.diff-fill {
		display: block;
		height: 100%;
		border-radius: inherit;
		background: color-mix(in srgb, #d89620 45%, var(--accent));
	}

	.tag-pills {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px;
		margin-bottom: 18px;
	}
	.tag-outline {
		display: inline-flex;
		align-items: center;
		padding: 3px 10px;
		border: 1px solid var(--line);
		border-radius: 6px;
		font-size: 11px;
		color: var(--muted);
		background: var(--bg);
	}
	.tag-outline.paused {
		background: var(--warm);
	}
	.tag-plus {
		width: 24px;
		height: 24px;
		padding: 0;
		border-radius: 50%;
		background: var(--soft);
		border: 1px solid var(--line);
		color: var(--accent);
		font-size: 14px;
		line-height: 1;
	}
	.tag-plus.active {
		background: var(--accent);
		color: var(--card);
		border-color: var(--accent);
	}

	.action-bar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		flex-wrap: wrap;
		padding-top: 14px;
		border-top: 1px solid var(--line);
	}
	.action-pills {
		display: flex;
		flex-wrap: wrap;
		gap: 10px;
	}
	.action-pill {
		border-radius: 999px;
		padding: 7px 16px;
		font-size: 12px;
		background: var(--soft);
		border: 1px solid var(--line);
		color: var(--fg);
	}
	.action-pill.saved {
		color: var(--accent);
		border-color: var(--accent);
		background: color-mix(in srgb, var(--accent) 10%, var(--soft));
	}
	.remove-pill.active {
		background: var(--warm);
		color: var(--muted);
	}
	.memory-indicator {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		color: var(--accent);
		font-size: 11px;
	}
	.memory-indicator small {
		color: var(--muted);
	}

	/* ── 2. 释义与内嵌例句卡片 ───────────────────────────────── */
	.word-card {
		padding: 26px 30px;
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

	.definitions.spaced {
		margin-top: 24px;
		padding-top: 24px;
		border-top: 1px solid var(--line);
	}
	.definitions-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 18px;
	}
	.pos-pill {
		display: inline-block;
		padding: 4px 12px;
		border-radius: 6px;
		background: var(--soft);
		border: 1px solid var(--line);
		color: var(--fg);
		font: italic 600 13px/1.3 var(--serif);
	}
	.defs-title {
		font-size: 12px;
		color: var(--muted);
		margin: 0;
		font-weight: 500;
	}

	.sense-block {
		margin: 20px 0;
	}
	.sense-block:first-of-type {
		margin-top: 0;
	}
	.sense-head {
		display: grid;
		grid-template-columns: 44px minmax(0, 1fr);
		align-items: start;
		gap: 12px;
	}
	.level-pill {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		height: 24px;
		padding: 0 9px;
		border-radius: 999px;
		font-size: 11px;
		font-weight: 700;
		background: var(--cefr-bg);
		color: var(--cefr-fg);
		border: 1px solid var(--cefr-border);
		margin-top: 3px;
	}
	.level-pill.rare {
		background: var(--soft);
		color: var(--muted);
		border-color: var(--line);
	}
	.english {
		font: 450 18px/1.65 var(--serif);
		white-space: pre-line;
		margin: 0;
		color: var(--fg);
	}
	.sense-example {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 12px;
		margin: 12px 0 16px 8px;
		border-left: 2px solid var(--line);
		padding: 2px 0 2px 16px;
	}
	.ex-main {
		min-width: 0;
		flex: 1;
	}
	.sense-example p {
		font: 450 16px/1.75 var(--serif);
		margin: 0;
	}
	.sense-example footer {
		font-size: 13px;
		color: var(--muted);
		margin-top: 5px;
	}
	.ex-sound {
		color: var(--accent);
		margin-top: -2px;
	}

	.translation-summary {
		margin-top: 22px;
		padding-top: 18px;
		border-top: 1px solid var(--line);
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 10px;
	}
	.translation-pill {
		display: inline-block;
		padding: 3px 10px;
		border-radius: 6px;
		background: var(--soft);
		border: 1px solid var(--line);
	}
	.translation-pill em {
		font: italic 600 12px var(--serif);
		color: var(--fg);
	}
	.translation-text {
		font-size: 15px;
		color: var(--fg);
		margin: 0;
		line-height: 1.7;
	}

	/* ── 3. 纵向拓展卡片流 ───────────────────────────────────── */
	.extra-card {
		border: 0;
		padding: 28px 34px;
	}
	.extra-title {
		font: italic 500 20px/1.25 var(--serif);
		color: var(--accent);
		margin: 0 0 14px;
		letter-spacing: -0.01em;
	}

	/* ── 留白沙龙 (obsidian) & 极地群青 (cobalt): 统一大圆角无框悬浮卡片格式 ── */
	:global(:root[data-edition='obsidian']) .hero-card,
	:global(:root[data-edition='obsidian']) .word-card,
	:global(:root[data-edition='cobalt']) .hero-card,
	:global(:root[data-edition='cobalt']) .word-card {
		border: 0;
		padding: 34px 38px;
	}
	:global(:root[data-edition='obsidian']) .extra-card,
	:global(:root[data-edition='cobalt']) .extra-card {
		border: 0;
		padding: 28px 34px;
	}

	/* ── 牛津辞书画报 (vermilion): 双线辞书眉头 + 紧凑分栏线框 ── */
	:global(:root[data-edition='vermilion']) .hero-card {
		border-top: 3px double var(--fg);
		padding: 28px 6px 22px;
	}
	:global(:root[data-edition='vermilion']) .hero-head h1 {
		font: 600 60px/1.05 var(--serif);
	}
	:global(:root[data-edition='vermilion']) .extra-card {
		border-top: 1px solid var(--line);
		padding: 22px 4px;
	}
	.mnemonic-body {
		font: 450 16px/1.8 var(--serif);
		margin: 0;
		color: var(--fg);
	}
	.kv-list {
		list-style: none;
		padding: 0;
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.kv-list li {
		display: grid;
		grid-template-columns: minmax(105px, auto) minmax(0, 1fr);
		align-items: baseline;
		gap: 18px;
		font-size: 14px;
	}
	.kv-label {
		font: italic 500 15px var(--serif);
		color: var(--muted);
	}
	.kv-val,
	.root-key {
		font: 500 16px var(--serif);
		color: var(--fg);
	}
	.root-val,
	.deriv-gloss {
		color: var(--muted);
		line-height: 1.65;
	}
	.deriv-gloss em {
		font: italic 500 14px var(--serif);
	}

	.deriv-root {
		display: flex;
		align-items: baseline;
		gap: 14px;
		flex-wrap: wrap;
		margin-bottom: 10px;
	}
	.deriv-root strong {
		font: 600 17px var(--serif);
		color: var(--fg);
	}
	.deriv-tree {
		list-style: none;
		padding: 0 0 0 6px;
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.deriv-tree li {
		position: relative;
		display: grid;
		grid-template-columns: 18px minmax(110px, auto) minmax(0, 1fr);
		align-items: baseline;
		gap: 10px;
		font-size: 14px;
	}
	.deriv-tree li:not(:last-child)::before {
		content: '';
		position: absolute;
		left: 4px;
		top: 13px;
		bottom: -14px;
		width: 1px;
		background: var(--line);
	}
	.tree-node {
		display: inline-block;
		width: 9px;
		height: 9px;
		border-radius: 50%;
		border: 1.5px solid var(--muted);
		background: var(--card);
		transform: translateY(-1px);
	}

	.word-jump {
		padding: 0;
		border: 0;
		background: transparent;
		font: 500 16px var(--serif);
		color: var(--fg);
		text-align: left;
		text-decoration: underline;
		text-decoration-color: var(--line);
		text-underline-offset: 3px;
	}
	.word-jump:hover {
		color: var(--accent);
		background: transparent;
	}

	.related-cloud {
		display: flex;
		flex-wrap: wrap;
		gap: 8px 14px;
	}
	.cloud-word {
		padding: 4px 10px;
		border: 1px solid var(--line);
		border-radius: 6px;
		background: var(--soft);
		font: 500 16px var(--serif);
		color: var(--fg);
	}
	.cloud-word:hover {
		border-color: var(--accent);
		color: var(--accent);
	}

	.collocation {
		display: flex;
		flex-direction: column;
		padding: 9px 0;
		border-bottom: 1px solid var(--line);
	}
	.collocation:last-child {
		border: 0;
		padding-bottom: 0;
	}
	.collocation span {
		font: 500 16px/1.7 var(--serif);
	}
	.collocation small {
		font-size: 12px;
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

	/* ── 右侧个人笔记栏 ──────────────────────────────────────── */
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

	@media (max-width: 1100px) {
		.word-layout {
			grid-template-columns: minmax(0, 1fr);
		}
		.hero-head h1 {
			font-size: 40px;
		}
	}
	@media (max-width: 600px) {
		.hero-card,
		.word-card,
		.extra-card {
			padding: 20px;
		}
		.hero-head h1 {
			font-size: 34px;
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
