<script lang="ts">
	import { onMount, tick } from 'svelte';
	import {
		reviewQueue,
		startReviewItem,
		requestHint,
		submitReview,
		dueLabel,
		errorMessage,
		type Hint,
		type ReviewFeedback,
		type ReviewItem
	} from '$lib/api';
	import Icon from '$lib/components/Icon.svelte';
	let items = $state<ReviewItem[]>([]);
	let index = $state(0);
	let reviewId = $state<number | null>(null);
	let answer = $state('');
	let hints = $state<Hint[]>([]);
	let feedback = $state<ReviewFeedback | null>(null);
	let loading = $state(true);
	let busy = $state(false);
	let error = $state('');
	let done = $state(false);
	let correct = $state(0);
	let completed = $state(0);
	let input: HTMLInputElement | undefined = $state();
	const item = $derived(items[index]);
	const qualities: Record<string, string> = {
		excellent: 'Recalled easily',
		good: 'Recalled with a nudge',
		hard: 'Took effort',
		poor: 'Barely held on',
		forgotten: 'Forgotten'
	};
	const hintLabels = ['First letter', 'English definition', 'Chinese definition'];
	async function load() {
		loading = true;
		error = '';
		index = 0;
		correct = 0;
		completed = 0;
		done = false;
		reviewId = null;
		feedback = null;
		try {
			items = await reviewQueue();
			if (items.length) await begin();
		} catch (e) {
			error = errorMessage(e);
		} finally {
			loading = false;
			await tick();
			input?.focus();
		}
	}
	async function begin() {
		const current = items[index];
		if (!current) return;
		busy = true;
		error = '';
		reviewId = null;
		answer = '';
		hints = [];
		feedback = null;
		try {
			reviewId = await startReviewItem(current);
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = false;
			await tick();
			input?.focus();
		}
	}
	async function submit(value: string | null) {
		if (reviewId === null || feedback || busy || (value !== null && !value.trim())) return;
		busy = true;
		error = '';
		try {
			feedback = await submitReview(reviewId, value);
			completed++;
			if (feedback.correct) correct++;
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = false;
		}
	}
	async function hint() {
		if (reviewId === null || feedback || busy || hints.length >= 3) return;
		busy = true;
		error = '';
		try {
			hints = [...hints, await requestHint(reviewId, hints.length + 1)];
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = false;
			await tick();
			input?.focus();
		}
	}
	async function next() {
		if (busy || !feedback || done) return;
		if (index + 1 >= items.length) {
			done = true;
			return;
		}
		index++;
		await begin();
	}
	function key(e: KeyboardEvent) {
		if (e.isComposing || e.repeat || e.ctrlKey || e.metaKey) return;
		if (
			feedback &&
			!done &&
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
	{#if loading}<div class="skeleton" role="status" aria-label="Loading review"></div>
	{:else if error && !items.length}<section class="panel empty">
			<h2>Review unavailable</h2>
			<p class="error" role="alert">{error}</p>
			<button onclick={load}>Reload</button>
		</section>
	{:else if !items.length}<section class="panel empty">
			<div class="empty-icon"><Icon name="check" size={28} /></div>
			<h2>Nothing due</h2>
			<p>Words you look up resurface here when they start to fade.</p>
			<a href="/" class="button primary">Look up a word <Icon name="arrow" size={16} /></a>
		</section>
	{:else if done}<section class="panel empty">
			<div class="empty-icon"><Icon name="check" size={28} /></div>
			<h2>Session complete</h2>
			<div class="summary">
				<div><span>{completed}</span>reviewed</div>
				<div><span>{correct}</span>recalled</div>
				<div><span>{completed - correct}</span>to revisit</div>
			</div>
			<a href="/" class="button primary">Done <Icon name="arrow" size={16} /></a><button
				class="ghost"
				onclick={load}>Next batch</button
			>
		</section>
	{:else if item}
		<div class="session-top">
			<span><strong>{index + 1}</strong> / {items.length}</span>
			<a href="/">Exit <Icon name="close" size={14} /></a>
		</div>
		<progress max={items.length} value={completed} aria-label="Review progress"></progress>
		<section class="panel question" aria-busy={busy}>
			<span class="kind">{item.example_id !== null ? 'Cloze' : 'Definition'}</span>
			<p class="prompt reading">{item.prompt}</p>
			{#if error}<p class="error" role="alert">{error}</p>{/if}
			{#if !reviewId}<div class="start-error">
					<p class="muted small">{busy ? 'Preparing…' : 'This item is not ready.'}</p>
					<button disabled={busy} onclick={begin}>Retry</button>
				</div>
			{:else if feedback}<div
					class="feedback rise"
					class:incorrect={!feedback.correct}
					role="status"
				>
					<span class="answer-icon"
						><Icon name={feedback.correct ? 'check' : 'book'} size={20} /></span
					>
					<div>
						<strong>{feedback.answer}</strong>
						<p>
							{qualities[feedback.quality]}{#if feedback.memory?.next_review_at}
								· next {dueLabel(feedback.memory.next_review_at)}{/if}
						</p>
					</div>
				</div>
				<div class="actions">
					<span class="muted small"
						>{feedback.hints_used
							? `${feedback.hints_used} hint${feedback.hints_used === 1 ? '' : 's'} used`
							: 'No hints'}</span
					><button class="primary" onclick={next} disabled={busy}
						>{index + 1 === items.length ? 'Finish' : 'Next'}<kbd>Enter</kbd></button
					>
				</div>
			{:else}<form
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
						<button type="button" class="ghost" disabled={busy} onclick={() => submit(null)}
							>Reveal</button
						><button class="primary" disabled={busy || !answer.trim()}>Check<kbd>Enter</kbd></button
						>
					</div>
				</form>{/if}
			<div class="hint-section">
				<button
					class="ghost hint-button"
					disabled={busy || !!feedback || !reviewId || hints.length >= 3}
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
	.start-error {
		text-align: center;
		padding: 18px;
	}
	@media (max-width: 600px) {
		.question {
			padding: 22px;
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
