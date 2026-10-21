<script lang="ts">
	import { onMount } from 'svelte';
	import { wordbooksList, errorMessage, type WordbookInfo } from '$lib/api';
	import {
		preferences,
		applyPreferences,
		DAILY_NEW_LIMITS,
		type StudyPreferences
	} from '$lib/preferences.svelte';
	import Icon from './Icon.svelte';

	let { onClose }: { onClose?: () => void } = $props();

	type Wordbook = StudyPreferences['wordbook'];
	const WORDBOOK_NAMES: Record<Wordbook, string> = {
		cet4: '大学英语四级 CET-4',
		cet6: '大学英语六级 CET-6',
		ky: '考研英语',
		toefl: 'TOEFL',
		ielts: 'IELTS',
		gre: 'GRE',
		gk: '高考英语',
		oxford: 'Oxford 3000',
		custom: '自定义词库',
		encountered: '查词记录'
	};
	const WORDBOOK_ORDER = Object.keys(WORDBOOK_NAMES) as Wordbook[];

	let books = $state<WordbookInfo[]>([]);
	let booksError = $state('');

	const bookRows = $derived(
		WORDBOOK_ORDER.map((id) => {
			const info = books.find((b) => b.id === id);
			return {
				id,
				name: WORDBOOK_NAMES[id],
				total: info?.total_words ?? null,
				learned: info?.learned_words ?? 0
			};
		})
	);

	onMount(async () => {
		try {
			books = await wordbooksList();
		} catch (e) {
			booksError = errorMessage(e);
		}
	});

	/** 每次修改学习偏好后立即持久化。 */
	function set<K extends keyof StudyPreferences>(key: K, value: StudyPreferences[K]) {
		preferences.study[key] = value;
		applyPreferences();
	}

	function toggleTab(index: number) {
		const tab = preferences.study.extensionTabs[index];
		tab.enabled = !tab.enabled;
		applyPreferences();
	}

	function moveTab(from: number, to: number) {
		const tabs = preferences.study.extensionTabs;
		if (to < 0 || to >= tabs.length || from === to) return;
		const next = [...tabs];
		const [item] = next.splice(from, 1);
		next.splice(to, 0, item);
		preferences.study.extensionTabs = next;
		applyPreferences();
	}

	let dragFrom = $state<number | null>(null);

	function handleKey(event: KeyboardEvent, index: number) {
		if (event.key === 'ArrowUp') {
			event.preventDefault();
			moveTab(index, index - 1);
		} else if (event.key === 'ArrowDown') {
			event.preventDefault();
			moveTab(index, index + 1);
		}
	}

	const cardModes = [
		['standard', '标准卡片'],
		['context', '语境卡片']
	] as const;
	const planStyles = [
		['standard', '标准'],
		['intensive', '强化'],
		['casual', '轻松']
	] as const;
	const priorities = [
		['mixed', '混合'],
		['new_first', '新词优先'],
		['review_first', '复习优先']
	] as const;
	const repeats = [
		['never', '不重复'],
		['thrice', '重复 3 次']
	] as const;
	const accents = [
		['US', '美音'],
		['UK', '英音']
	] as const;
	const rates = [
		[0.8, '0.8x'],
		[1, '1x'],
		[1.2, '1.2x']
	] as const;
	const exampleStyles = [
		['standard', '标准例句'],
		['exam', '真题例句']
	] as const;
	const exampleOrders = [
		['first', '首条'],
		['random', '随机']
	] as const;
</script>

{#snippet toggle(key: keyof StudyPreferences, label: string, last = false)}
	<div class="setting-row" class:last>
		<h3 id="study-{key}">{label}</h3>
		<button
			class="switch"
			role="switch"
			aria-checked={preferences.study[key] === true}
			aria-labelledby="study-{key}"
			onclick={() => set(key, !preferences.study[key] as never)}><span></span></button
		>
	</div>
{/snippet}

<div class="study-settings">
	{#if onClose}
		<div class="panel-head">
			<h2>学习设置</h2>
			<button class="icon-btn" aria-label="关闭学习设置" onclick={onClose}
				><Icon name="close" size={16} /></button
			>
		</div>
	{/if}

	<section class="panel">
		<div class="section-heading"><h2>词书与计划</h2></div>
		<div class="books" role="radiogroup" aria-label="词书">
			{#each bookRows as b}
				<button
					class="book"
					role="radio"
					aria-checked={preferences.study.wordbook === b.id}
					class:chosen={preferences.study.wordbook === b.id}
					onclick={() => set('wordbook', b.id)}
				>
					<span class="book-name">{b.name}</span>
					{#if b.total !== null}
						<span class="book-count">{b.learned.toLocaleString()} / {b.total.toLocaleString()}</span>
						<span class="book-bar" aria-hidden="true"
							><span style:width="{b.total ? Math.min(100, (b.learned / b.total) * 100) : 0}%"
							></span></span
						>
					{:else}<span class="book-count">—</span>{/if}
				</button>
			{/each}
		</div>
		{#if booksError}<p class="muted small">词书进度暂不可用：{booksError}</p>{/if}
		<p class="muted small blurb">
			自定义词库收录你在查词或学习时收藏的单词。想整理收藏，可以到
			<a href="/library?filter=bookmarked">单词本 · 已收藏</a>。
		</p>
		<div class="setting-row">
			<h3>卡片类型</h3>
			<div class="segmented">
				{#each cardModes as [key, label]}<button
						aria-pressed={preferences.study.cardMode === key}
						class:chosen={preferences.study.cardMode === key}
						onclick={() => set('cardMode', key)}>{label}</button
					>{/each}
			</div>
		</div>
		<div class="setting-row">
			<h3>计划风格</h3>
			<div class="segmented">
				{#each planStyles as [key, label]}<button
						aria-pressed={preferences.study.planStyle === key}
						class:chosen={preferences.study.planStyle === key}
						onclick={() => set('planStyle', key)}>{label}</button
					>{/each}
			</div>
		</div>
		<div class="setting-row">
			<h3>每日新词</h3>
			<div class="segmented">
				{#each DAILY_NEW_LIMITS as n}<button
						aria-pressed={preferences.study.dailyNewLimit === n}
						class:chosen={preferences.study.dailyNewLimit === n}
						onclick={() => set('dailyNewLimit', n)}>{n}</button
					>{/each}
			</div>
		</div>
		<div class="setting-row last">
			<h3>学习顺序</h3>
			<div class="segmented">
				{#each priorities as [key, label]}<button
						aria-pressed={preferences.study.priority === key}
						class:chosen={preferences.study.priority === key}
						onclick={() => set('priority', key)}>{label}</button
					>{/each}
			</div>
		</div>
	</section>

	<section class="panel">
		<div class="section-heading"><h2>发音</h2></div>
		{@render toggle('autoPronounceWord', '单词自动发音')}
		<div class="setting-row">
			<h3>重复播放</h3>
			<div class="segmented">
				{#each repeats as [key, label]}<button
						aria-pressed={preferences.study.pronounceRepeat === key}
						class:chosen={preferences.study.pronounceRepeat === key}
						onclick={() => set('pronounceRepeat', key)}>{label}</button
					>{/each}
			</div>
		</div>
		{@render toggle('autoPronounceExample', '例句自动发音')}
		{@render toggle('autoPronounceContext', '语境卡片自动发音')}
		<div class="setting-row">
			<h3>口音</h3>
			<div class="segmented">
				{#each accents as [key, label]}<button
						aria-pressed={preferences.study.accent === key}
						class:chosen={preferences.study.accent === key}
						onclick={() => set('accent', key)}>{label}</button
					>{/each}
			</div>
		</div>
		<div class="setting-row last">
			<h3>语速</h3>
			<div class="segmented">
				{#each rates as [key, label]}<button
						aria-pressed={preferences.study.speechRate === key}
						class:chosen={preferences.study.speechRate === key}
						onclick={() => set('speechRate', key)}>{label}</button
					>{/each}
			</div>
		</div>
	</section>

	<section class="panel">
		<div class="section-heading"><h2>例句</h2></div>
		<div class="setting-row">
			<h3>例句类型</h3>
			<div class="segmented">
				{#each exampleStyles as [key, label]}<button
						aria-pressed={preferences.study.exampleStyle === key}
						class:chosen={preferences.study.exampleStyle === key}
						onclick={() => set('exampleStyle', key)}>{label}</button
					>{/each}
			</div>
		</div>
		<div class="setting-row last">
			<h3>显示方式</h3>
			<div class="segmented">
				{#each exampleOrders as [key, label]}<button
						aria-pressed={preferences.study.exampleOrder === key}
						class:chosen={preferences.study.exampleOrder === key}
						onclick={() => set('exampleOrder', key)}>{label}</button
					>{/each}
			</div>
		</div>
		<p class="muted small footnote">真题例句目前支持大学英语四级、六级，及考研英语真题。</p>
	</section>

	<section class="panel">
		<div class="section-heading"><h2>卡片显示与测验</h2></div>
		{@render toggle('autoSplitSyllables', '自动拆分音节')}
		{@render toggle('fadeIn', '卡片渐显')}
		{@render toggle('hideChinese', '隐藏中文释义')}
		{@render toggle('inSessionQuiz', '学完后拼写测验', true)}
	</section>

	<section class="panel">
		<div class="section-heading"><h2>扩展页签</h2></div>
		<p class="muted small blurb">勾选要显示的页签，拖动 ≡ 或用 ↑ ↓ 键调整顺序。</p>
		<ul class="tab-list">
			{#each preferences.study.extensionTabs as tab, i (tab.id)}
				<li
					class:dragging={dragFrom === i}
					ondragover={(e) => e.preventDefault()}
					ondrop={(e) => {
						e.preventDefault();
						if (dragFrom !== null) moveTab(dragFrom, i);
						dragFrom = null;
					}}
				>
					<button
						class="check"
						role="checkbox"
						aria-checked={tab.enabled}
						onclick={() => toggleTab(i)}
						><span class="box" class:on={tab.enabled}
							>{#if tab.enabled}<Icon name="check" size={12} />{/if}</span
						>{tab.label}</button
					>
					<button
						class="handle"
						draggable="true"
						aria-label="移动 {tab.label}（第 {i + 1} 项，共 {preferences.study.extensionTabs
							.length} 项），按上下方向键调整"
						ondragstart={(e) => {
							dragFrom = i;
							e.dataTransfer?.setData('text/plain', tab.id);
						}}
						ondragend={() => (dragFrom = null)}
						onkeydown={(e) => handleKey(e, i)}>≡</button
					>
				</li>
			{/each}
		</ul>
		{@render toggle('autoReturnToExamples', '切换单词时自动返回例句', true)}
	</section>
</div>

<style>
	.study-settings {
		display: flex;
		flex-direction: column;
		gap: 20px;
	}
	.panel-head {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}
	.panel-head h2 {
		font-size: 18px;
		margin: 0;
	}
	.icon-btn {
		display: inline-flex;
		padding: 6px;
		border: 0;
		background: transparent;
		color: var(--muted);
	}
	.setting-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 20px;
		padding: 16px 0;
		border-bottom: 1px solid var(--line);
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
	.switch {
		position: relative;
		width: 38px;
		height: 22px;
		padding: 0;
		border: 1px solid var(--line);
		border-radius: 99px;
		background: var(--bg);
		flex-shrink: 0;
	}
	.switch span {
		position: absolute;
		top: 2px;
		left: 2px;
		width: 16px;
		height: 16px;
		border-radius: 50%;
		background: var(--muted);
		transition: transform 0.16s var(--ease);
	}
	.switch[aria-checked='true'] {
		background: var(--soft);
		border-color: var(--accent);
	}
	.switch[aria-checked='true'] span {
		transform: translateX(16px);
		background: var(--accent);
	}
	.books {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
		gap: 8px;
		margin-bottom: 12px;
	}
	.book {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 4px;
		padding: 10px 12px;
		text-align: left;
		background: var(--bg);
		border: 1px solid var(--line);
		border-radius: 10px;
		color: var(--fg);
	}
	.book.chosen {
		border-color: var(--accent);
		background: var(--soft);
	}
	.book-name {
		font-size: 12px;
		font-weight: 500;
	}
	.book-count {
		font-size: 10px;
		color: var(--muted);
	}
	.book-bar {
		display: block;
		width: 100%;
		height: 3px;
		background: var(--line);
		border-radius: 3px;
		overflow: hidden;
	}
	.book-bar span {
		display: block;
		height: 100%;
		background: var(--accent);
	}
	.blurb {
		line-height: 1.7;
		margin: 0 0 6px;
	}
	.footnote {
		margin: 14px 0 0;
	}
	.tab-list {
		list-style: none;
		padding: 0;
		margin: 6px 0 0;
		border-bottom: 1px solid var(--line);
	}
	.tab-list li {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 4px 0;
		border-top: 1px solid var(--line);
	}
	.tab-list li.dragging {
		opacity: 0.5;
	}
	.check {
		display: flex;
		align-items: center;
		gap: 10px;
		border: 0;
		background: transparent;
		color: var(--fg);
		font-size: 13px;
		padding: 8px 0;
	}
	.box {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 16px;
		height: 16px;
		border: 1px solid var(--line);
		border-radius: 4px;
		color: var(--card);
	}
	.box.on {
		background: var(--accent);
		border-color: var(--accent);
	}
	.handle {
		border: 0;
		background: transparent;
		color: var(--muted);
		font-size: 16px;
		padding: 4px 10px;
		cursor: grab;
	}
	@media (max-width: 600px) {
		.setting-row {
			align-items: flex-start;
			flex-direction: column;
			gap: 12px;
		}
		.setting-row:has(.switch) {
			flex-direction: row;
			align-items: center;
		}
		.segmented {
			width: 100%;
		}
		.segmented button {
			flex: 1;
		}
	}
</style>
