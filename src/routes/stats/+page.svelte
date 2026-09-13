<script lang="ts">
	import { onMount } from 'svelte';
	import Icon from '$lib/components/Icon.svelte';
	import {
		statsSummary,
		fadingWords,
		activityDays,
		dueLabel,
		errorMessage,
		type StatsSummary,
		type FadingWord,
		type ActivityDay
	} from '$lib/api';
	let stats = $state<StatsSummary | null>(null);
	let fading = $state<FadingWord[]>([]);
	let days = $state<ActivityDay[]>([]);
	let loading = $state(true);
	let error = $state('');
	const maxActivity = $derived(Math.max(1, ...days.map((d) => d.lookups + d.reviews)));
	const totalLookups = $derived(days.reduce((n, d) => n + d.lookups, 0));
	const totalReviews = $derived(days.reduce((n, d) => n + d.reviews, 0));
	const active = $derived(stats ? stats.learning + stats.familiar + stats.stable : 0);
	const stablePercent = $derived(active && stats ? Math.round((stats.stable / active) * 100) : 0);
	async function load() {
		loading = true;
		error = '';
		try {
			[stats, fading, days] = await Promise.all([statsSummary(), fadingWords(8), activityDays()]);
		} catch (e) {
			error = errorMessage(e);
		} finally {
			loading = false;
		}
	}
	onMount(() => {
		void load();
	});
</script>

<main class="page insights">
	<h1>Stats</h1>
	{#if loading}<div class="skeleton" role="status" aria-label="Loading stats"></div>
	{:else if error}<p class="error" role="alert">{error}</p>
		<button onclick={load}>Retry</button>
	{:else if stats}
		<dl class="metric-grid">
			{#each [{ label: 'Encountered', value: stats.encountered }, { label: 'Familiar', value: stats.familiar + stats.stable }, { label: 'Stable', value: stats.stable }, { label: 'Due now', value: stats.due_now }] as m}<div
					class="metric"
				>
					<dt>{m.label}</dt>
					<dd>{m.value.toLocaleString()}</dd>
				</div>{/each}
		</dl>
		<section class="panel activity">
			<div class="section-heading">
				<h2>Last 14 days</h2>
				<div class="legend">
					<span class="lookup-key">{totalLookups} lookups</span><span class="review-key"
						>{totalReviews} reviews</span
					>
				</div>
			</div>
			<div
				class="chart"
				role="img"
				aria-label={`${totalLookups} lookups and ${totalReviews} reviews over the last 14 days.`}
			>
				{#each days as d, i}<div
						class="chart-column"
						title={`${d.date}: ${d.lookups} lookups, ${d.reviews} reviews`}
					>
						<div class="bar-track">
							<div class="bar" style:height={`${((d.lookups + d.reviews) / maxActivity) * 100}%`}>
								<div class="review-bar" style:flex={d.reviews}></div>
								<div class="lookup-bar" style:flex={d.lookups}></div>
							</div>
						</div>
						<span
							>{i === 0 || i === 13 || i % 3 === 0 ? d.date.slice(5).replace('-', '/') : '·'}</span
						>
					</div>{/each}
			</div>
			{#if !totalLookups && !totalReviews}<p class="chart-empty">No activity yet.</p>{/if}
		</section>
		<div class="insight-grid">
			<section class="panel distribution">
				<div class="section-heading"><h2>Stages</h2></div>
				<div class="distribution-content">
					<div
						class="donut"
						style={`--stable:${active ? (stats.stable / active) * 100 : 0}%;--familiar:${active ? ((stats.stable + stats.familiar) / active) * 100 : 0}%`}
					>
						<div><strong>{stablePercent}<small>%</small></strong><span>stable</span></div>
					</div>
					<dl>
						{#each [{ label: 'Learning', value: stats.learning, color: 'var(--line)' }, { label: 'Familiar', value: stats.familiar, color: 'var(--kraft)' }, { label: 'Stable', value: stats.stable, color: 'var(--accent)' }] as row}<div
							>
								<dt><i style:background={row.color}></i>{row.label}</dt>
								<dd>{row.value}</dd>
							</div>{/each}
					</dl>
				</div>
				{#if stats.ignored}<p class="muted small">{stats.ignored} paused, not counted.</p>{/if}
			</section>
			<section class="panel fading">
				<div class="section-heading">
					<h2>Fading</h2>
					{#if fading.length}<a href="/review">Review</a>{/if}
				</div>
				{#if fading.length}<ul>
						{#each fading as word}<li>
								<a href={`/?word=${encodeURIComponent(word.word)}`}
									>{word.display}{#if word.high_priority}<span
											class="priority"
											title="High priority"
										></span>{/if}</a
								><small>{dueLabel(word.next_review_at)}</small>
							</li>{/each}
					</ul>{:else}<div class="fading-empty">
						<Icon name="check" size={24} />
						<p>Nothing due</p>
					</div>{/if}
			</section>
		</div>
	{/if}
</main>

<style>
	.insights h1 {
		font-size: 24px;
		margin-bottom: 22px;
	}
	.metric-grid {
		margin: 0 0 22px;
	}
	.activity {
		margin-bottom: 22px;
		padding: 24px;
	}
	.activity h2 {
		font-size: 14px;
	}
	.legend {
		display: flex;
		gap: 18px;
		color: var(--muted);
		font-size: 10px;
	}
	.legend span::before {
		content: '';
		display: inline-block;
		width: 7px;
		height: 7px;
		margin-right: 6px;
		border-radius: 2px;
		background: var(--accent);
	}
	.legend .review-key::before {
		background: var(--kraft);
	}
	.chart {
		display: flex;
		gap: 14px;
		height: 150px;
		align-items: stretch;
	}
	.chart-column {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		justify-content: flex-end;
		align-items: center;
		gap: 10px;
	}
	.bar-track {
		display: flex;
		align-items: flex-end;
		justify-content: center;
		width: 100%;
		height: 122px;
		background: repeating-linear-gradient(to top, var(--line) 0 1px, transparent 1px 41px);
	}
	.bar {
		display: flex;
		flex-direction: column;
		width: 62%;
		max-width: 30px;
		border-radius: 4px 4px 0 0;
		overflow: hidden;
		min-height: 2px;
		background: var(--line);
	}
	.lookup-bar {
		background: var(--accent);
	}
	.review-bar {
		background: var(--kraft);
	}
	.chart-column > span {
		font-size: 9px;
		color: var(--muted);
	}
	.chart-empty {
		color: var(--muted);
		font-size: 11px;
		text-align: center;
		margin: 16px 0 0;
	}
	.insight-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 22px;
	}
	.insight-grid h2 {
		font-size: 14px;
	}
	.distribution-content {
		display: flex;
		align-items: center;
		gap: 28px;
		margin: 10px 0 20px;
	}
	.donut {
		width: 118px;
		height: 118px;
		flex-shrink: 0;
		border-radius: 50%;
		background: conic-gradient(
			var(--accent) 0 var(--stable),
			var(--kraft) var(--stable) var(--familiar),
			var(--line) var(--familiar) 100%
		);
		display: grid;
		place-items: center;
		transform: rotate(-90deg);
	}
	.donut > div {
		width: 98px;
		height: 98px;
		border-radius: 50%;
		background: var(--card);
		display: flex;
		align-items: center;
		justify-content: center;
		flex-direction: column;
		transform: rotate(90deg);
	}
	.donut strong {
		font: 500 27px var(--serif);
	}
	.donut strong small {
		font-size: 14px;
	}
	.donut span {
		font-size: 9px;
		color: var(--muted);
		margin-top: 3px;
	}
	.distribution dl {
		flex: 1;
		margin: 0;
		min-width: 0;
	}
	.distribution dl > div {
		display: flex;
		justify-content: space-between;
		gap: 12px;
		margin: 12px 0;
		font-size: 11px;
	}
	.distribution dt {
		color: var(--muted);
		white-space: nowrap;
	}
	.distribution i {
		display: inline-block;
		width: 6px;
		height: 6px;
		border-radius: 50%;
		margin-right: 8px;
	}
	.distribution dd {
		margin: 0;
	}
	.distribution > p {
		font-size: 10px;
		margin: 0;
	}
	.fading ul {
		list-style: none;
		padding: 0;
		margin: 0;
	}
	.fading li {
		display: flex;
		align-items: center;
		justify-content: space-between;
		border-bottom: 1px solid var(--line);
		padding: 9px 0;
		gap: 12px;
	}
	.fading li:last-child {
		border: 0;
	}
	.fading li a {
		font: 500 17px var(--serif);
		text-decoration: none;
		color: var(--fg);
	}
	.fading li a:hover {
		color: var(--accent);
	}
	.fading li small {
		font-size: 10px;
		color: var(--muted);
		white-space: nowrap;
	}
	.priority {
		display: inline-block;
		width: 4px;
		height: 4px;
		margin-left: 7px;
		background: var(--accent);
		border-radius: 50%;
	}
	.fading-empty {
		text-align: center;
		color: var(--accent);
		padding: 26px 0;
	}
	.fading-empty p {
		font-size: 12px;
		color: var(--muted);
		margin: 12px 0 0;
	}
	@media (max-width: 1100px) {
		.insight-grid {
			grid-template-columns: 1fr;
		}
		.distribution-content {
			max-width: 340px;
		}
		.chart {
			gap: 8px;
		}
	}
	@media (max-width: 520px) {
		.activity {
			padding: 18px;
		}
		.chart {
			gap: 5px;
		}
		.chart-column > span {
			font-size: 8px;
		}
		.donut {
			width: 108px;
			height: 108px;
		}
		.donut > div {
			width: 90px;
			height: 90px;
		}
	}
</style>
