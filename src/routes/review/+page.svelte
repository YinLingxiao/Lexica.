<script lang="ts">
	import { onMount, tick } from 'svelte';
	import {
		reviewGroup,
		startReviewItem,
		requestHint,
		submitReview,
		practiceCheck,
		practiceHint,
		aiConfigGet,
		aiGenerateExamples,
		dueLabel,
		errorMessage,
		type AiExample,
		type Hint,
		type ReviewFeedback,
		type ReviewGroupEntry,
		type ReviewItem
	} from '$lib/api';
	import Icon from '$lib/components/Icon.svelte';
	import ReviewBrowseCard from '$lib/components/ReviewBrowseCard.svelte';

	type Phase = 'loading' | 'error' | 'empty' | 'browse' | 'practice' | 'done';

	interface CardFeedback {
		correct: boolean;
		answer: string;
		/** true = first formal attempt (recorded); false = read-only retry. */
		formal: boolean;
		quality: string | null;
		next: string | null;
		hints_used: number;
	}

	let phase = $state<Phase>('loading');
	let error = $state('');
	let group = $state<ReviewGroupEntry[]>([]);
	let browseIdx = $state(0);

	// 练习态：题目在"开始练习"那一刻固定（AI 迟到结果只进缓存，不改已固定题目）。
	let prompts = $state<ReviewItem[]>([]);
	let order = $state<number[]>([]); // 打乱后的组内下标；答错/揭示追加到队尾
	let pos = $state(0);
	let formalDone = $state<boolean[]>([]);
	let solved = $state<boolean[]>([]);
	let mastered = $state(0); // 首轮即答对的词数
	let retried = $state(0); // 经重考才答对的词数

	let reviewId = $state<number | null>(null);
	let answer = $state('');
	let hints = $state<Hint[]>([]);
	let feedback = $state<CardFeedback | null>(null);
	let busy = $state(false);
	let cardError = $state('');
	let input: HTMLInputElement | undefined = $state();

	// AI（可选）：浏览期间为整组词批量生成填空句；不可用时只用释义题。
	let aiNote = $state('');
	let aiNeedsSetup = $state(false);
	let aiResults: AiExample[] = [];

	const qualities: Record<string, string> = {
		excellent: 'Recalled easily',
		good: 'Recalled with a nudge',
		hard: 'Took effort',
		poor: 'Barely held on',
		forgotten: 'Forgotten'
	};
	const hintLabels = ['First letter', 'English definition', 'Chinese definition'];
	const kindLabel: Record<string, string> = {
		cloze: 'Cloze',
		definition: 'Definition',
		definition_zh: 'Chinese definition'
	};

	const current = $derived(order[pos] !== undefined ? group[order[pos]] : undefined);
	const currentPrompt = $derived(order[pos] !== undefined ? prompts[order[pos]] : undefined);
	// 只统计首轮已失败且尚未答对的词；未作答的新词不属于“待重考”。
	const toRevisit = $derived(formalDone.filter((done, i) => done && !solved[i]).length);
	const solvedCount = $derived(solved.filter(Boolean).length);

	async function load() {
		phase = 'loading';
		error = '';
		try {
			group = await reviewGroup();
			if (!group.length) {
				phase = 'empty';
				return;
			}
			formalDone = group.map(() => false);
			solved = group.map(() => false);
			browseIdx = 0;
			aiNote = '';
			aiNeedsSetup = false;
			aiResults = [];
			phase = 'browse';
			kickOffAi();
		} catch (e) {
			error = errorMessage(e);
			phase = 'error';
		}
	}

	/** 复习填空句统一由 AI 生成；词典例句只供浏览阅读。 */
	function kickOffAi() {
		void (async () => {
			try {
				const items = group.map((g) => ({
					word_id: g.item.word_id,
					sense_id: g.item.sense_id
				}));
				if (!items.length) return;
				const cfg = await aiConfigGet();
				if (!cfg.enabled || !cfg.has_key) {
					aiNeedsSetup = true;
					return;
				}
				aiNeedsSetup = false;
				aiNote = 'Generating AI example sentences…';
				const examples = await aiGenerateExamples(items);
				aiResults = examples; // 迟到结果只存这里；后端已写入 SQLite 缓存
				aiNote = examples.length
					? `AI examples ready for ${examples.length} word${examples.length === 1 ? '' : 's'}.`
					: '';
			} catch (e) {
				aiNote = `AI examples unavailable — using local prompts (${errorMessage(e)}).`;
			}
		})();
	}

	function startPractice() {
		// 固定题目：AI 有效例句（已挖空、已验证）优先于本地释义；未完成的词用本地题目。
		prompts = group.map((g) => {
			const ai = aiResults.find((a) => a.word_id === g.item.word_id);
			if (ai && ai.sentence.trim()) {
				return {
					...g.item,
					kind: 'cloze' as const,
					source: 'ai' as const,
					prompt: ai.sentence,
					sense_id: ai.sense_id ?? g.item.sense_id,
					example_id: null
				};
			}
			return g.item;
		});
		// Fisher–Yates：一次性打乱本组顺序。
		order = group.map((_, i) => i);
		for (let i = order.length - 1; i > 0; i--) {
			const j = Math.floor(Math.random() * (i + 1));
			[order[i], order[j]] = [order[j], order[i]];
		}
		pos = 0;
		mastered = 0;
		retried = 0;
		phase = 'practice';
		void beginCard();
	}

	async function beginCard() {
		const gi = order[pos];
		if (gi === undefined) return;
		busy = true;
		cardError = '';
		reviewId = null;
		answer = '';
		hints = [];
		feedback = null;
		try {
			// 只在首轮答题时创建正式 review；重考走只读接口，不重复记录。
			if (!formalDone[gi]) {
				const item = prompts[gi];
				reviewId = await startReviewItem({
					word_id: item.word_id,
					prompt: item.prompt,
					kind: item.kind,
					source: item.source,
					sense_id: item.sense_id,
					example_id: item.example_id
				});
			}
		} catch (e) {
			cardError = errorMessage(e);
		} finally {
			busy = false;
			await tick();
			input?.focus();
		}
	}

	/**
	 * 创建正式 review 失败时重新创建；题目已开始后的临时错误只清除提示，
	 * 保留 reviewId 和已经使用的提示，避免重复记录或降低提示计数。
	 */
	async function retryCard() {
		const gi = order[pos];
		if (gi === undefined) return;
		if (!formalDone[gi] && reviewId === null) {
			await beginCard();
			return;
		}
		cardError = '';
		await tick();
		input?.focus();
	}

	function requeue() {
		const gi = order[pos];
		if (gi !== undefined) order = [...order, gi]; // 追加队尾；再次答错继续追加
	}

	async function submit(value: string | null) {
		const gi = order[pos];
		if (gi === undefined || !currentPrompt || feedback || busy) return;
		if (value !== null && !value.trim()) return;
		busy = true;
		cardError = '';
		try {
			if (!formalDone[gi]) {
				// 首轮答题：正式记录 + 记忆更新（现有机制，含拼写容错与质量映射）。
				if (reviewId === null) throw new Error('This item is not ready.');
				const fb: ReviewFeedback = await submitReview(reviewId, value);
				formalDone[gi] = true;
				feedback = {
					correct: fb.correct,
					answer: fb.answer,
					formal: true,
					quality: fb.quality,
					next: fb.memory?.next_review_at ?? null,
					hints_used: fb.hints_used
				};
				if (fb.correct) {
					solved[gi] = true;
					mastered++;
				} else {
					requeue();
				}
			} else {
				// 组内重考：只读判分，不重复计入正式复习/记忆评分。
				const res = await practiceCheck(currentPrompt.word_id, value);
				feedback = {
					correct: res.correct,
					answer: group[gi].entry.display,
					formal: false,
					quality: null,
					next: null,
					hints_used: hints.length
				};
				if (res.correct) {
					solved[gi] = true;
					retried++;
				} else {
					requeue();
				}
			}
		} catch (e) {
			cardError = errorMessage(e);
		} finally {
			busy = false;
		}
	}

	async function hint() {
		const gi = order[pos];
		if (gi === undefined || !currentPrompt || feedback || busy || hints.length >= 3) return;
		busy = true;
		cardError = '';
		try {
			const next = hints.length + 1;
			const h = formalDone[gi]
				? await practiceHint(currentPrompt.word_id, currentPrompt.sense_id, next)
				: await requestHint(reviewId as number, next);
			hints = [...hints, h];
		} catch (e) {
			cardError = errorMessage(e);
		} finally {
			busy = false;
			await tick();
			input?.focus();
		}
	}

	async function next() {
		if (busy || !feedback) return;
		if (pos + 1 >= order.length) {
			phase = 'done';
			return;
		}
		pos++;
		await beginCard();
	}

	function key(e: KeyboardEvent) {
		if (e.isComposing || e.repeat || e.ctrlKey || e.metaKey) return;
		if (phase === 'browse') {
			if (e.key === 'ArrowRight') browseIdx = Math.min(browseIdx + 1, group.length - 1);
			else if (e.key === 'ArrowLeft') browseIdx = Math.max(browseIdx - 1, 0);
			return;
		}
		if (
			feedback &&
			!busy &&
			e.key === 'Enter' &&
			!(e.target as HTMLElement)?.matches('button,a,input,textarea,select')
		) {
			e.preventDefault();
			void next();
		}
	}

	onMount(() => {
		void load();
	});
</script>

<svelte:window onkeydown={key} />
<main class="page review-page">
	{#if phase === 'loading'}<div class="skeleton" role="status" aria-label="Loading review"></div>
	{:else if phase === 'error'}<section class="panel empty">
			<h2>Review unavailable</h2>
			<p class="error" role="alert">{error}</p>
			<button onclick={load}>Reload</button>
		</section>
	{:else if phase === 'empty'}<section class="panel empty">
			<div class="empty-icon"><Icon name="check" size={28} /></div>
			<h2>Nothing due</h2>
			<p>Words you look up resurface here when they start to fade.</p>
			<a href="/" class="button primary">Look up a word <Icon name="arrow" size={16} /></a>
		</section>
	{:else if phase === 'browse' && group.length}
		<div class="session-top">
			<span>Browse · <strong>{browseIdx + 1}</strong> / {group.length}</span>
			<a href="/">Exit <Icon name="close" size={14} /></a>
		</div>
		<progress max={group.length} value={browseIdx + 1} aria-label="Browse progress"></progress>
		<section class="panel browse-panel">
			{#key browseIdx}<ReviewBrowseCard entry={group[browseIdx].entry} />{/key}
		</section>
		{#if aiNeedsSetup}<p class="ai-note">
				Need richer example sentences? <a href="/settings#ai-examples">Configure DeepSeek</a>
			</p>{:else if aiNote}<p class="ai-note" role="status">{aiNote}</p>{/if}
		<div class="browse-actions">
			<button onclick={() => (browseIdx = Math.max(0, browseIdx - 1))} disabled={browseIdx === 0}
				>Previous</button
			>
			<div class="browse-right">
				<button
					onclick={() => (browseIdx = Math.min(group.length - 1, browseIdx + 1))}
					disabled={browseIdx === group.length - 1}>Next</button
				><button
					class="primary"
					onclick={startPractice}
					disabled={browseIdx < group.length - 1}>Start practice</button
				>
			</div>
		</div>
	{:else if phase === 'practice' && current && currentPrompt}
		<div class="session-top">
			<span><strong>{solvedCount}</strong> / {group.length} recalled</span>
			<span class="revisit">{toRevisit} to revisit</span>
			<a href="/">Exit <Icon name="close" size={14} /></a>
		</div>
		<!-- 进度按"已答对词数"计：重考追加队列不会推高进度。 -->
		<progress max={group.length} value={solvedCount} aria-label="Review progress"></progress>
		<section class="panel question" aria-busy={busy}>
			<span class="kind"
				>{kindLabel[currentPrompt.kind]}{currentPrompt.source === 'ai' ? ' · AI' : ''}</span
			>
			<p class="prompt reading">{currentPrompt.prompt}</p>
			{#if cardError}<p class="error" role="alert">{cardError}</p>
					<button class="ghost retry" onclick={retryCard}>Retry</button>{/if}
			{#if feedback}
				<div class="feedback rise" class:incorrect={!feedback.correct} role="status">
					<span class="answer-icon"
						><Icon name={feedback.correct ? 'check' : 'book'} size={20} /></span
					>
					<div>
						<strong>{feedback.answer}</strong>
						<p>
							{feedback.formal
								? `${qualities[feedback.quality ?? 'forgotten']}${feedback.next
										? ` · next ${dueLabel(feedback.next)}`
										: ''}`
								: feedback.correct
									? 'Correct — recalled on retry'
									: 'Not quite — this word will come back'}
						</p>
					</div>
				</div>
				<div class="actions">
					<span class="muted small"
						>{feedback.hints_used
							? `${feedback.hints_used} hint${feedback.hints_used === 1 ? '' : 's'} used`
							: 'No hints'}</span
					><button class="primary" onclick={next} disabled={busy}
						>{pos + 1 === order.length ? 'Finish' : 'Next'}<kbd>Enter</kbd></button
					>
				</div>
			{:else}
				<form
					onsubmit={(e) => {
						e.preventDefault();
						void submit(answer);
					}}
				>
					<label for="review-answer">Which word?</label><input
						id="review-answer"
						bind:this={input}
						bind:value={answer}
						placeholder="Type the word…"
						autocomplete="off"
						autocapitalize="off"
						spellcheck="false"
						disabled={busy}
						onkeydown={(e) => {
							if (e.isComposing && e.key === 'Enter') e.preventDefault();
							if (e.key === 'Escape' && !e.isComposing) {
								e.preventDefault();
								answer = '';
							}
						}}
					/>
					<div class="actions">
						<button
							type="button"
							class="ghost"
							disabled={busy || (!formalDone[order[pos]] && reviewId === null)}
							onclick={() => submit(null)}
							>Reveal</button
						><button
							class="primary"
							disabled={busy || !answer.trim() || (!formalDone[order[pos]] && reviewId === null)}
							>Check<kbd>Enter</kbd></button
						>
					</div>
				</form>
			{/if}
			<div class="hint-section">
				<button
					class="ghost hint-button"
					disabled={
						busy ||
						!!feedback ||
						hints.length >= 3 ||
						(!formalDone[order[pos]] && reviewId === null)
					}
					onclick={hint}
					><Icon name="spark" size={14} />{hints.length >= 3
						? 'No hints left'
						: hintLabels[hints.length]}<span>{Math.min(hints.length + 1, 3)} / 3</span></button
				>
				{#if hints.length}<ol class="hints">
						{#each hints as h, i}<li>
								<span class="hint-step">{i + 1}</span><span class:mask={h.kind === 'mask'}
									>{h.text || 'Not available for this entry.'}</span
								>
							</li>{/each}
					</ol>{/if}
			</div>
		</section>
	{:else if phase === 'done'}
		<section class="panel empty">
			<div class="empty-icon"><Icon name="check" size={28} /></div>
			<h2>Group complete</h2>
			<div class="summary">
				<div><span>{mastered}</span>recalled first try</div>
				<div><span>{retried}</span>needed practice</div>
				<div><span>{group.length}</span>in group</div>
			</div>
			<a href="/" class="button primary">Done <Icon name="arrow" size={16} /></a><button
				class="ghost"
				onclick={load}>Next group</button
			>
		</section>
	{/if}
</main>

<style>
	.review-page {
		max-width: 780px;
	}
	.session-top {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 12px;
		font-size: 11px;
		color: var(--muted);
		margin-bottom: 10px;
	}
	.session-top strong {
		font: 500 17px var(--serif);
		color: var(--accent);
		margin-right: 3px;
	}
	.session-top a {
		display: flex;
		align-items: center;
		gap: 7px;
		text-decoration: none;
		color: var(--muted);
	}
	.revisit {
		color: var(--muted);
	}
	progress {
		width: 100%;
		height: 4px;
		border: 0;
		appearance: none;
		display: block;
		background: var(--line);
		border-radius: 10px;
		margin-bottom: 22px;
	}
	progress::-webkit-progress-bar {
		background: var(--line);
		border-radius: 10px;
	}
	progress::-webkit-progress-value {
		background: var(--accent);
		border-radius: 10px;
	}
	progress::-moz-progress-bar {
		background: var(--accent);
		border-radius: 10px;
	}
	.browse-panel {
		padding: 30px 34px;
	}
	.ai-note {
		color: var(--muted);
		font-size: 11px;
		margin: 12px 2px 0;
	}
	.browse-actions {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 12px;
		margin-top: 18px;
	}
	.browse-actions button {
		font-size: 12px;
	}
	.browse-right {
		display: flex;
		gap: 8px;
	}
	.question {
		padding: 30px 34px;
	}
	.kind {
		font-size: 9px;
		letter-spacing: 1.8px;
		text-transform: uppercase;
		color: var(--muted);
	}
	.prompt {
		font: 450 23px/1.8 var(--serif);
		white-space: pre-line;
		overflow-wrap: anywhere;
		min-height: 84px;
		margin: 20px 0 28px;
	}
	.retry {
		margin: -14px 0 16px;
		font-size: 11px;
	}
	form label {
		display: block;
		font-size: 11px;
		color: var(--muted);
		margin-bottom: 9px;
	}
	form input {
		width: 100%;
		height: 52px;
		font: 500 18px var(--serif);
	}
	.actions {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 12px;
		margin-top: 18px;
	}
	.actions button {
		font-size: 12px;
	}
	.feedback {
		display: flex;
		align-items: center;
		gap: 15px;
		padding: 20px;
		background: var(--soft);
		border-radius: 12px;
	}
	.feedback.incorrect {
		background: var(--warm);
	}
	.answer-icon {
		color: var(--accent);
	}
	.feedback strong {
		font: 500 26px var(--serif);
		overflow-wrap: anywhere;
	}
	.feedback p {
		color: var(--muted);
		font-size: 11px;
		margin: 5px 0 0;
	}
	.hint-section {
		border-top: 1px solid var(--line);
		margin-top: 30px;
		padding-top: 12px;
	}
	.hint-button {
		font-size: 11px;
		padding: 8px 0;
		color: var(--accent);
	}
	.hint-button span {
		font-size: 10px;
		color: var(--muted);
	}
	.hints {
		padding: 0;
		list-style: none;
		margin: 6px 0 0;
	}
	.hints li {
		display: flex;
		gap: 12px;
		padding: 12px 0;
		border-top: 1px solid var(--line);
		font-size: 14px;
	}
	.hint-step {
		color: var(--muted);
		font: italic 500 14px var(--serif);
	}
	.mask {
		letter-spacing: 3px;
		font: 500 18px var(--serif);
	}
	.summary {
		display: flex;
		justify-content: center;
		gap: 40px;
		margin: 28px 0;
		font-size: 11px;
		color: var(--muted);
	}
	.summary span {
		display: block;
		font: 500 28px var(--serif);
		color: var(--fg);
		margin-bottom: 5px;
	}
	@media (max-width: 600px) {
		.browse-panel,
		.question {
			padding: 22px;
		}
		.browse-actions {
			flex-wrap: wrap;
		}
		.prompt {
			font-size: 20px;
		}
		.actions kbd {
			display: none;
		}
		.summary {
			gap: 24px;
		}
	}
</style>
