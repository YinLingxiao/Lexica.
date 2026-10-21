<script module lang="ts">
	// 跨卡片记住上次查看的扩展页签（关闭“自动返回例句”时使用）。
	let lastTab = 'examples';
</script>

<script lang="ts">
	import { onMount } from 'svelte';
	import type { Example, RelatedWordItem, StudyCardEntry, StudyGrade } from '$lib/api';
	import { preferences, applyPreferences } from '$lib/preferences.svelte';
	import SyllableHeadword from './SyllableHeadword.svelte';
	import { speakText } from '$lib/tts';
	import Icon from './Icon.svelte';
	import HighlightedText from './HighlightedText.svelte';

	let {
		card,
		busy = false,
		progressIndex = 0,
		progressTotal = 1,
		onToggleBookmark,
		onIgnoreWord,
		onGradeWord,
		onOpenSettings,
		onSelectWord
	}: {
		card: StudyCardEntry;
		busy?: boolean;
		progressIndex?: number;
		progressTotal?: number;
		onToggleBookmark: () => void;
		onIgnoreWord: () => void;
		onGradeWord: (grade: StudyGrade) => void;
		onOpenSettings: () => void;
		onSelectWord?: (word: string) => void;
	} = $props();

	const s = $derived(preferences.study);
	const entry = $derived(card.entry);
	const primaryPos = $derived(
		entry.senses.find((sense) => sense.pos?.trim())?.pos?.replace(/\.$/, '') ?? 'word'
	);
	const chineseSummary = $derived.by(() => {
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

	const allExamples = $derived(entry.senses.flatMap((sense) => sense.examples));
	// 每张卡片挂载时固定一次随机数，避免重渲染时例句跳动。
	const seed = Math.random();
	const example = $derived.by((): Example | null => {
		if (s.exampleStyle === 'exam' && entry.exam_example) return entry.exam_example;
		if (!allExamples.length) return entry.exam_example ?? null;
		return s.exampleOrder === 'random'
			? allExamples[Math.floor(seed * allExamples.length)]
			: allExamples[0];
	});

	let zhShown = $state(false);
	let defsShown = $state(false);
	let tabStrip: HTMLDivElement | undefined = $state();
	const definitionsVisible = $derived(s.cardMode === 'standard' || defsShown);

	const tabs = $derived([
		{ id: 'examples', label: '例句' },
		...s.extensionTabs.filter((t) => t.enabled).map((t) => ({ id: t.id as string, label: t.label }))
	]);
	let tab = $state(preferences.study.autoReturnToExamples ? 'examples' : lastTab);
	const activeTab = $derived(tabs.some((t) => t.id === tab) ? tab : 'examples');
	function selectTab(id: string) {
		tab = id;
		lastTab = id;
	}

	function scrollTabsRight() {
		if (!tabStrip) return;
		const maxScroll = tabStrip.scrollWidth - tabStrip.clientWidth;
		if (maxScroll <= 4) {
			const idx = tabs.findIndex((t) => t.id === activeTab);
			const next = tabs[(idx + 1) % tabs.length];
			if (next) selectTab(next.id);
			return;
		}
		if (tabStrip.scrollLeft >= maxScroll - 8) {
			tabStrip.scrollTo({ left: 0, behavior: 'smooth' });
		} else {
			tabStrip.scrollBy({ left: 160, behavior: 'smooth' });
		}
	}

	function toggleExampleStyle() {
		preferences.study.exampleStyle = s.exampleStyle === 'exam' ? 'standard' : 'exam';
		applyPreferences();
	}

	function relatedFor(id: string): RelatedWordItem[] {
		const plain = (words: string[]) => words.map((word) => ({ word, pos: null, gloss: '' }));
		switch (id) {
			case 'derivatives':
				return entry.derivatives?.length ? entry.derivatives : plain(entry.word_family);
			case 'synonyms':
				return entry.synonyms_detailed?.length ? entry.synonyms_detailed : plain(entry.synonyms);
			case 'antonyms':
				return entry.antonyms_detailed?.length ? entry.antonyms_detailed : plain(entry.antonyms);
			case 'lookalikes':
				return entry.lookalikes ?? [];
			default:
				return [];
		}
	}

	const TICK_COUNT = 60;
	const filledTicks = $derived(
		Math.max(
			1,
			Math.min(TICK_COUNT, Math.round(((progressIndex + 1) / Math.max(1, progressTotal)) * TICK_COUNT))
		)
	);

	function speakWord() {
		speakText(entry.word, {
			accent: s.accent,
			rate: s.speechRate,
			repeat: s.pronounceRepeat === 'thrice' ? 3 : 1
		});
	}
	function speakSentence(text: string) {
		speakText(text, { accent: s.accent, rate: s.speechRate });
	}

	onMount(() => {
		if (s.cardMode === 'context' && s.autoPronounceContext && example) speakSentence(example.text);
		else if (s.autoPronounceWord) speakWord();
		else if (s.autoPronounceExample && example) speakSentence(example.text);
	});
</script>

<article class="study-card" class:fade={s.fadeIn}>
	<div class="card-top-bar">
		<span class="mode-tag">{card.is_new ? '新词' : '复习'}</span>
		<button
			class="icon-btn dots-btn"
			aria-label="学习设置"
			title="学习设置"
			onclick={onOpenSettings}><Icon name="dots" size={18} /></button
		>
	</div>

	<header class="head">
		<h2 class="headword"><SyllableHeadword word={entry.display} split={s.autoSplitSyllables} /></h2>
		<div class="phonetic-row">
			{#if entry.phonetic}<span class="phonetic">{entry.phonetic}</span>{/if}
			<button class="icon-btn sound-btn" aria-label="朗读单词" title="朗读单词" onclick={speakWord}
				><Icon name="sound" size={18} /></button
			>
		</div>
	</header>

	<div class="card-body-grid">
		<div class="card-left-pane">
			{#if s.cardMode === 'context' && example && !definitionsVisible}
				<div class="context-card">
					<div class="example-header">
						<span class="example-pill"><em>context</em><span>语境</span></span>
						<button
							class="icon-btn sound-btn"
							aria-label="朗读例句"
							onclick={() => speakSentence(example.text)}><Icon name="sound" size={16} /></button
						>
					</div>
					<p class="example-en reading"><HighlightedText text={example.text} word={entry.word} /></p>
					<button class="reveal block" onclick={() => (defsShown = true)}>点击显示完整释义</button>
				</div>
			{/if}

			{#if definitionsVisible}
				<div class="senses-block">
					<div class="pos-row">
						<span class="pos-pill">{primaryPos}</span>
					</div>
					<div class="senses-scroll" class:has-fade={entry.senses.length > 3}>
						{#each entry.senses as sense}
							{@const lvl = sense.level?.trim() || 'R'}
							<div class="sense-item">
								<span class="level-pill" class:rare={lvl === 'R'}>{lvl}</span>
								<p class="english reading">
									{sense.english_definition || sense.chinese_definition || ''}
								</p>
							</div>
						{:else}
							<p class="muted small">暂无释义。</p>
						{/each}
					</div>
				</div>
			{/if}
		</div>

		<div class="card-right-pane">
			{#if tabs.length && definitionsVisible}
				<div class="tabs-bar">
					<div class="tabs-scroll" role="tablist" aria-label="扩展内容" bind:this={tabStrip}>
						{#each tabs as t}<button
								role="tab"
								aria-selected={activeTab === t.id}
								class:chosen={activeTab === t.id}
								onclick={() => selectTab(t.id)}>{t.label}</button
							>{/each}
					</div>
					<button
						class="tabs-more"
						type="button"
						aria-label="更多扩展栏"
						title="横向滚动扩展栏"
						onclick={scrollTabsRight}><Icon name="chevrons-right" size={16} /></button
					>
				</div>

				<div class="tab-body" role="tabpanel">
					{#if activeTab === 'examples'}
						<div class="examples-panel">
							{#if example}
								<div class="example-header">
									<button
										type="button"
										class="example-pill"
										title="切换标准/真题例句"
										onclick={toggleExampleStyle}
									>
										<em>example</em><span>{s.exampleStyle === 'exam' ? '真题' : '标准'}</span>
									</button>
									<button
										class="icon-btn sound-btn"
										aria-label="朗读例句"
										onclick={() => speakSentence(example.text)}><Icon name="sound" size={16} /></button
									>
								</div>
								<p class="example-en reading">
									<HighlightedText text={example.text} word={entry.word} />
								</p>
								{#if example.translation}
									{#if s.hideChinese && !zhShown}
										<button class="reveal" onclick={() => (zhShown = true)}>点击显示中文译文</button>
									{:else}
										<p class="example-zh">{example.translation}</p>
									{/if}
								{/if}
							{:else}
								<p class="muted small">暂无例句。</p>
							{/if}

							{#if chineseSummary}
								<div class="translation-block">
									<span class="translation-pill"><em>translation</em></span>
									{#if s.hideChinese && !zhShown}
										<button class="reveal" onclick={() => (zhShown = true)}>点击显示中文释义</button>
									{:else}
										<p class="translation-zh">{chineseSummary}</p>
									{/if}
								</div>
							{/if}
						</div>
					{:else if activeTab === 'mnemonic'}
						<div class="ext-card">
							{#if entry.mnemonic}
								<p class="mnemonic-text">{entry.mnemonic}</p>
							{:else}
								<p class="muted small">暂无助记。</p>
							{/if}
						</div>
					{:else if activeTab === 'roots'}
						<div class="ext-card">
							{#if entry.roots?.length}
								<ul class="ext-grid">
									{#each entry.roots as r}
										<li>
											<strong class="root-part">{r.part}</strong>
											<span class="root-meaning">{r.meaning}</span>
										</li>
									{/each}
								</ul>
							{:else}
								<p class="muted small">暂无词根词缀。</p>
							{/if}
						</div>
					{:else if activeTab === 'derivatives'}
						{@const items = relatedFor('derivatives')}
						<div class="ext-card">
							<div class="derivative-head">
								<strong>{entry.display}</strong>
								{#if chineseSummary}<span class="muted">{chineseSummary}</span>{/if}
							</div>
							{#if items.length}
								<ul class="ext-grid derivatives-list">
									{#each items as r}
										<li>
											<span class="bullet" aria-hidden="true">◦</span>
											{#if onSelectWord}
												<button type="button" class="word-jump" onclick={() => onSelectWord(r.word)}
													>{r.word}</button
												>
											{:else}
												<strong>{r.word}</strong>
											{/if}
											<span class="gloss">
												{#if r.pos}<em>{r.pos}</em> {/if}{r.gloss}
											</span>
										</li>
									{/each}
								</ul>
							{:else}
								<p class="muted small">暂无派生词。</p>
							{/if}
						</div>
					{:else if activeTab === 'forms'}
						<div class="ext-card">
							{#if entry.forms?.length}
								<ul class="ext-grid">
									{#each entry.forms as f}
										<li>
											<span class="form-label">{f.label}</span>
											<strong class="form-value">{f.form}</strong>
										</li>
									{/each}
								</ul>
							{:else}
								<p class="muted small">暂无词形变化。</p>
							{/if}
						</div>
					{:else}
						{@const items = relatedFor(activeTab)}
						<div class="ext-card">
							{#if items.length}
								<ul class="ext-grid">
									{#each items as r}
										<li>
											{#if onSelectWord}
												<button type="button" class="word-jump" onclick={() => onSelectWord(r.word)}
													>{r.word}</button
												>
											{:else}
												<strong>{r.word}</strong>
											{/if}
											<span class="gloss">
												{#if r.pos}<em>{r.pos}</em> {/if}{r.gloss}
											</span>
										</li>
									{/each}
								</ul>
							{:else}
								<p class="muted small">暂无内容。</p>
							{/if}
						</div>
					{/if}
				</div>
			{/if}
		</div>
	</div>

	<!-- 右下侧三联悬浮圆钮（收藏 ☆ / 斩词 🗑 / 记忆小脑 🧠） -->
	<aside class="floating-dock" aria-label="单词操作">
		<button
			type="button"
			class="fab"
			aria-pressed={card.bookmarked}
			aria-label={card.bookmarked ? '取消收藏' : '收藏'}
			title={card.bookmarked ? '取消收藏' : '收藏单词'}
			onclick={onToggleBookmark}
		>
			<Icon name={card.bookmarked ? 'star-filled' : 'star'} size={19} />
		</button>
		<button
			type="button"
			class="fab"
			aria-label="不再学习这个词"
			title="标为熟知（不再安排学习）"
			disabled={busy}
			onclick={onIgnoreWord}
		>
			<Icon name="trash" size={18} />
		</button>
		<div class="brain-group" role="group" aria-label="掌握度评分">
			<div class="sub-grades">
				<button
					type="button"
					class="sub-grade"
					disabled={busy}
					title="模糊（快捷键 2）"
					onclick={() => onGradeWord('hazy')}>模糊</button
				>
				<button
					type="button"
					class="sub-grade"
					disabled={busy}
					title="忘记（快捷键 3）"
					onclick={() => onGradeWord('forgot')}>忘记</button
				>
			</div>
			<button
				type="button"
				class="fab brain-fab"
				aria-label="认识"
				title="认识并进入下一词（Space / 1）"
				disabled={busy}
				onclick={() => onGradeWord('know')}
			>
				<Icon name="brain" size={20} />
			</button>
		</div>
	</aside>

	<!-- 底部细密刻度进度条 -->
	<div
		class="tick-bar"
		role="progressbar"
		aria-valuemin={0}
		aria-valuemax={progressTotal}
		aria-valuenow={progressIndex + 1}
		aria-label="本组学习进度"
	>
		{#each { length: TICK_COUNT } as _, idx}
			<span class="tick" class:filled={idx < filledTicks}></span>
		{/each}
	</div>
</article>

<style>
	.study-card {
		position: relative;
		padding-bottom: 18px;
	}
	.study-card.fade {
		animation: card-fade 0.36s var(--ease);
	}
	@keyframes card-fade {
		from {
			opacity: 0;
			transform: translateY(4px);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.study-card.fade {
			animation: none;
		}
	}

	.card-top-bar {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 4px;
	}
	.mode-tag {
		font-size: 10px;
		color: var(--muted);
		border: 1px solid var(--line);
		border-radius: 4px;
		padding: 2px 7px;
	}
	.dots-btn {
		margin-left: auto;
	}

	.head {
		text-align: center;
		padding: 10px 0 22px;
	}
	.headword {
		font: 600 52px/1.08 var(--serif);
		letter-spacing: -0.025em;
		margin: 0 0 12px;
		color: var(--fg);
		overflow-wrap: anywhere;
	}
	.phonetic-row {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: 8px;
	}
	.phonetic {
		color: var(--muted);
		font-size: 16px;
		letter-spacing: 0.01em;
	}
	.icon-btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 32px;
		height: 32px;
		padding: 0;
		border: 0;
		border-radius: 8px;
		background: transparent;
		color: var(--muted);
	}
	.icon-btn:hover:not(:disabled) {
		color: var(--fg);
		background: var(--soft);
	}
	.sound-btn {
		color: var(--accent);
	}

	/* ── 英英分义与 CEFR 等级胶囊 ────────────────────────────── */
	.senses-block {
		margin-top: 6px;
		padding-right: 56px;
	}
	.pos-row {
		margin-bottom: 12px;
	}
	.pos-pill {
		display: inline-block;
		padding: 3px 11px;
		border-radius: 6px;
		background: var(--soft);
		border: 1px solid var(--line);
		color: var(--fg);
		font: italic 600 13px/1.4 var(--serif);
	}
	.senses-scroll {
		display: flex;
		flex-direction: column;
		gap: 14px;
		max-height: 220px;
		overflow-y: auto;
		padding-right: 4px;
	}
	.senses-scroll.has-fade {
		mask-image: linear-gradient(to bottom, #000 82%, transparent 100%);
		-webkit-mask-image: linear-gradient(to bottom, #000 82%, transparent 100%);
		padding-bottom: 14px;
	}
	.sense-item {
		display: grid;
		grid-template-columns: 42px minmax(0, 1fr);
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
		letter-spacing: 0.02em;
		background: var(--cefr-bg);
		color: var(--cefr-fg);
		border: 1px solid var(--cefr-border);
		margin-top: 2px;
	}
	.level-pill.rare {
		background: var(--soft);
		color: var(--muted);
		border-color: var(--line);
	}
	.english {
		font: 450 17px/1.62 var(--serif);
		color: var(--fg);
		margin: 0;
		white-space: pre-line;
	}

	/* ── 横向 8 大拓展栏导航 ────────────────────────────────── */
	.tabs-bar {
		display: flex;
		align-items: center;
		gap: 6px;
		margin-top: 22px;
		padding: 10px 56px 10px 0;
		border-top: 1px solid var(--line);
		border-bottom: 1px solid var(--line);
	}
	.tabs-scroll {
		display: flex;
		align-items: center;
		gap: 6px;
		overflow-x: auto;
		scrollbar-width: none;
		flex: 1;
		min-width: 0;
	}
	.tabs-scroll::-webkit-scrollbar {
		display: none;
	}
	.tabs-scroll button {
		flex-shrink: 0;
		font-size: 13px;
		padding: 5px 11px;
		border: 0;
		border-radius: 6px;
		background: transparent;
		color: var(--muted);
		font-weight: 500;
	}
	.tabs-scroll button.chosen {
		background: color-mix(in srgb, var(--accent) 13%, var(--soft));
		color: var(--accent);
		font-weight: 600;
	}
	.tabs-more {
		flex-shrink: 0;
		padding: 4px 6px;
		border: 0;
		background: transparent;
		color: var(--accent);
	}

	/* ── 拓展栏内容区 ───────────────────────────────────────── */
	.tab-body {
		margin-top: 16px;
		padding-right: 60px;
		min-height: 150px;
	}
	.examples-panel {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.example-header {
		display: flex;
		align-items: center;
		gap: 10px;
	}
	.example-pill {
		display: inline-flex;
		align-items: center;
		padding: 0;
		border: 1px solid var(--line);
		border-radius: 6px;
		overflow: hidden;
		background: var(--card);
		font-size: 11px;
	}
	.example-pill em {
		padding: 3px 9px;
		font: italic 600 12px var(--serif);
		color: var(--fg);
		background: var(--soft);
	}
	.example-pill span {
		padding: 3px 9px;
		color: var(--muted);
		background: var(--card);
		font-weight: 500;
	}
	.example-en {
		font: 450 18px/1.7 var(--serif);
		margin: 0;
		color: var(--fg);
	}
	.example-zh {
		font-size: 14px;
		color: var(--muted);
		margin: 0;
		line-height: 1.65;
	}
	.translation-block {
		margin-top: 8px;
		padding-top: 12px;
		border-top: 1px dashed var(--line);
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 8px;
	}
	.translation-pill {
		display: inline-block;
		padding: 2px 9px;
		border-radius: 6px;
		background: var(--soft);
		border: 1px solid var(--line);
	}
	.translation-pill em {
		font: italic 600 12px var(--serif);
		color: var(--fg);
	}
	.translation-zh {
		font-size: 14px;
		color: var(--fg);
		margin: 0;
		line-height: 1.65;
	}

	.ext-card,
	.context-card {
		background: var(--soft);
		border: 1px solid var(--line);
		border-radius: 12px;
		padding: 18px 20px;
	}
	.context-card {
		margin-top: 12px;
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.mnemonic-text {
		font: 450 15px/1.8 var(--serif);
		margin: 0;
		color: var(--fg);
	}
	.ext-grid {
		list-style: none;
		padding: 0;
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.ext-grid li {
		display: grid;
		grid-template-columns: minmax(92px, auto) minmax(0, 1fr);
		align-items: baseline;
		gap: 16px;
		font-size: 14px;
	}
	.derivatives-list li {
		grid-template-columns: 14px minmax(110px, auto) minmax(0, 1fr);
		gap: 10px;
	}
	.derivative-head {
		display: flex;
		align-items: baseline;
		gap: 14px;
		flex-wrap: wrap;
		padding-bottom: 10px;
		margin-bottom: 10px;
		border-bottom: 1px solid var(--line);
	}
	.derivative-head strong,
	.root-part,
	.form-value,
	.ext-grid strong {
		font: 500 16px var(--serif);
		color: var(--fg);
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
	.bullet {
		color: var(--muted);
	}
	.root-meaning,
	.gloss {
		color: var(--muted);
		line-height: 1.6;
	}
	.gloss em,
	.form-label {
		font: italic 500 14px var(--serif);
		color: var(--muted);
	}

	.reveal {
		font-size: 12px;
		padding: 5px 11px;
		border: 1px dashed var(--line);
		background: transparent;
		color: var(--muted);
	}
	.reveal.block {
		display: flex;
		width: 100%;
		margin-top: 6px;
		padding: 10px;
	}

	/* ── 右下侧三联悬浮圆钮 ─────────────────────────────────── */
	.floating-dock {
		position: absolute;
		right: 0;
		bottom: 26px;
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: 10px;
		z-index: 5;
	}
	.fab {
		width: 44px;
		height: 44px;
		border-radius: 50%;
		padding: 0;
		background: var(--card);
		border: 1px solid var(--line);
		color: var(--fg);
		box-shadow: var(--shadow);
	}
	.fab:hover:not(:disabled) {
		background: var(--soft);
		border-color: var(--accent);
	}
	.fab[aria-pressed='true'] {
		background: var(--soft);
		color: var(--accent);
		border-color: var(--accent);
	}
	.brain-fab {
		background: color-mix(in srgb, var(--accent) 14%, var(--card));
		color: var(--accent);
		border-color: color-mix(in srgb, var(--accent) 45%, var(--line));
	}
	.brain-group {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.sub-grades {
		display: flex;
		gap: 6px;
		opacity: 0;
		pointer-events: none;
		transform: translateX(6px);
		transition:
			opacity 0.16s var(--ease),
			transform 0.16s var(--ease);
	}
	.brain-group:hover .sub-grades,
	.brain-group:focus-within .sub-grades {
		opacity: 1;
		pointer-events: auto;
		transform: translateX(0);
	}
	.sub-grade {
		padding: 6px 10px;
		font-size: 11px;
		border-radius: 999px;
		background: var(--card);
		border: 1px solid var(--line);
		color: var(--muted);
		box-shadow: var(--shadow);
	}
	.sub-grade:hover:not(:disabled) {
		color: var(--fg);
		background: var(--soft);
	}

	/* ── 底部细密刻度进度条 ─────────────────────────────────── */
	.tick-bar {
		display: grid;
		grid-template-columns: repeat(60, 1fr);
		gap: 2px;
		margin-top: 22px;
		padding-top: 10px;
		height: 16px;
		align-items: end;
	}
	.tick {
		display: block;
		width: 100%;
		height: 6px;
		border-radius: 1px;
		background: var(--line);
		transition: background 0.16s var(--ease);
	}
	.tick.filled {
		background: var(--accent);
	}

	/* ── 3 套系统主题风格适配 (StudyWordCard) ───────────── */

	/* 留白沙龙 (obsidian) & 极地群青 (cobalt): 统一无框柔和内舱卡片 */
	:global(:root[data-edition='obsidian']) .ext-card,
	:global(:root[data-edition='cobalt']) .ext-card {
		border: 0;
		border-radius: 18px;
		padding: 22px 24px;
	}

	/* 牛津辞书 (vermilion): 古典双线与斜体衬线页签 */
	:global(:root[data-edition='vermilion']) .head {
		border-bottom: 3px double var(--line);
		margin-bottom: 18px;
	}
	:global(:root[data-edition='vermilion']) .headword {
		font: 600 58px/1.06 var(--serif);
	}
	:global(:root[data-edition='vermilion']) .ext-card {
		border-radius: 2px;
		background: transparent;
	}

	@media (max-width: 600px) {
		.headword {
			font-size: 36px;
		}
		.senses-block,
		.tabs-bar,
		.tab-body {
			padding-right: 50px;
		}
		.sub-grades {
			opacity: 1;
			pointer-events: auto;
			transform: none;
		}
	}
</style>
