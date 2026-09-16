<div class="ai-wait" role="status" aria-live="polite">
	<div class="heading">
		<span class="spinner" aria-hidden="true"></span>
		<span>Writing an example sentence…</span>
	</div>
	<div class="lines" aria-hidden="true">
		<span></span>
		<span></span>
	</div>
</div>

<style>
	.ai-wait {
		width: 100%;
		min-width: 0;
		padding: 4px 0;
	}
	.heading {
		display: flex;
		align-items: center;
		gap: 10px;
		color: var(--muted);
		font-size: 12px;
	}
	.spinner {
		width: 16px;
		height: 16px;
		flex: none;
		border: 2px solid color-mix(in srgb, var(--accent) 24%, transparent);
		border-top-color: var(--accent);
		border-radius: 50%;
		animation: spin 0.9s linear infinite;
	}
	.lines {
		display: grid;
		gap: 10px;
		margin-top: 18px;
	}
	.lines span {
		display: block;
		height: 12px;
		border-radius: 6px;
		background: linear-gradient(
			90deg,
			var(--line) 20%,
			color-mix(in srgb, var(--fg) 13%, var(--line)) 50%,
			var(--line) 80%
		);
		background-size: 220% 100%;
		animation: gleam 1.7s ease-in-out infinite;
	}
	.lines span:first-child {
		width: 87%;
	}
	.lines span:last-child {
		width: 61%;
		animation-delay: 0.15s;
	}
	@keyframes spin {
		to { transform: rotate(360deg); }
	}
	@keyframes gleam {
		to { background-position: -220% 0; }
	}
</style>
