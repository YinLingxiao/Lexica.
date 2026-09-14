<script lang="ts">
	import '../app.css';
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import Icon from '$lib/components/Icon.svelte';
	import { isPreview } from '$lib/api';
	import { preferences, loadPreferences, applyPreferences } from '$lib/preferences.svelte';
	import { dictionarySetup, runDictionarySetup } from '$lib/dictionary-setup.svelte';
	let { children } = $props();
	let ready = $state(false);
	const navigation = [
		{ href: '/', label: 'Search', icon: 'search' },
		{ href: '/library', label: 'Words', icon: 'bookmark' },
		{ href: '/review', label: 'Review', icon: 'review' },
		{ href: '/stats', label: 'Stats', icon: 'chart' },
		{ href: '/settings', label: 'Settings', icon: 'settings' }
	];
	const title = $derived(navigation.find((n) => n.href === page.url.pathname)?.label ?? 'Search');
	onMount(() => {
		loadPreferences();
		ready = true;
		void runDictionarySetup().catch(() => {});
	});
	$effect(() => {
		if (ready) applyPreferences();
	});
	/** Expands the incoming theme as a circle centred on the toggle that was clicked. */
	function toggleTheme(event: MouseEvent) {
		const dark =
			preferences.theme === 'dark' ||
			(preferences.theme === 'system' && matchMedia('(prefers-color-scheme: dark)').matches);
		const next = dark ? 'light' : 'dark';
		const root = document.documentElement;
		const apply = () => {
			preferences.theme = next;
			applyPreferences();
		};
		if (!document.startViewTransition || matchMedia('(prefers-reduced-motion: reduce)').matches) {
			apply();
			return;
		}
		const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
		const x = box.left + box.width / 2;
		const y = box.top + box.height / 2;
		root.style.setProperty('--reveal-x', `${x}px`);
		root.style.setProperty('--reveal-y', `${y}px`);
		root.style.setProperty(
			'--reveal-r',
			`${Math.hypot(Math.max(x, innerWidth - x), Math.max(y, innerHeight - y))}px`
		);
		root.dataset.revealing = '';
		document.startViewTransition(apply).finished.finally(() => delete root.dataset.revealing);
	}
</script>

<svelte:head><title>{title} · Lexica</title></svelte:head>
<a class="skip" href="#content">Skip to content</a>
<div class="app-shell">
	<aside class="sidebar">
		<a href="/" class="brand" aria-label="Lexica home">lexica<span>.</span></a>
		<nav aria-label="Main">
			{#each navigation as n}<a
					href={n.href}
					class:active={page.url.pathname === n.href}
					aria-current={page.url.pathname === n.href ? 'page' : undefined}
					><Icon name={n.icon} size={17} /><span>{n.label}</span></a
				>{/each}
		</nav>
		<button class="theme" aria-label="Toggle color theme" onclick={toggleTheme}
			><Icon name={preferences.theme === 'dark' ? 'sun' : 'moon'} size={16} /></button
		>
	</aside>
	<div class="workspace">
		{#if ready && isPreview()}<div class="preview-note">
				Read-only browser preview — use the desktop app to save your data.
			</div>{/if}
		<div id="content" tabindex="-1">{@render children()}</div>
	</div>
</div>
{#if dictionarySetup.active}<div
		class="setup-overlay"
		role="dialog"
		aria-modal="true"
		aria-labelledby="setup-title"
	>
		<div class="setup-card panel">
			<div class="setup-icon"><Icon name="download" size={28} /></div>
			<h2 id="setup-title">
				{dictionarySetup.phase === 'error' ? 'Dictionary setup failed' : 'Setting up dictionary'}
			</h2>
			<p class="muted">{dictionarySetup.message || 'Preparing…'}</p>
			{#if dictionarySetup.phase !== 'error'}<div class="setup-bar" aria-hidden="true">
					<div
						class="setup-fill"
						class:indeterminate={dictionarySetup.progress == null}
						style:width={dictionarySetup.progress != null
							? `${Math.round(dictionarySetup.progress * 100)}%`
							: undefined}
					></div>
				</div>
				{#if dictionarySetup.progress != null}<span class="setup-pct"
						>{Math.round(dictionarySetup.progress * 100)}%</span
					>{/if}
			{:else}<button class="primary" onclick={() => void runDictionarySetup(true).catch(() => {})}
					>Retry</button
				>{/if}
			<p class="setup-note">
				ECDICT (~200 MB download). Stored in the app data folder. Import may take several minutes.
			</p>
		</div>
	</div>{/if}

<style>
	.app-shell {
		display: flex;
		min-height: 100vh;
	}
	.sidebar {
		width: 188px;
		position: fixed;
		inset: 0 auto 0 0;
		background: var(--sidebar);
		border-right: 1px solid var(--line);
		padding: 26px 14px 18px;
		display: flex;
		flex-direction: column;
		z-index: 30;
	}
	.brand {
		padding: 0 12px 26px;
		text-decoration: none;
		font: italic 450 26px/1 var(--serif);
		letter-spacing: -0.02em;
		color: var(--fg);
	}
	.brand span {
		color: var(--accent);
	}
	nav {
		display: grid;
		gap: 2px;
	}
	nav a {
		display: flex;
		align-items: center;
		gap: 12px;
		text-decoration: none;
		padding: 10px 12px;
		color: var(--muted);
		border-radius: 8px;
		font-size: 13px;
		transition:
			color 0.16s var(--ease),
			background 0.16s var(--ease);
	}
	nav a.active {
		color: var(--accent);
		background: var(--soft);
	}
	nav a:hover {
		color: var(--fg);
	}
	.theme {
		margin: auto 0 0 auto;
		border: 0;
		background: transparent;
		color: var(--muted);
		padding: 9px;
	}
	.theme:hover {
		background: var(--soft);
		border-color: transparent;
		color: var(--fg);
	}
	.theme :global(svg) {
		transition: transform 0.4s var(--ease);
	}
	.theme:hover :global(svg) {
		transform: rotate(-18deg);
	}
	.workspace {
		margin-left: 188px;
		min-width: 0;
		flex: 1;
	}
	.preview-note {
		font-size: 11px;
		color: var(--muted);
		background: var(--warm);
		padding: 7px 28px;
		text-align: center;
	}
	.skip {
		position: fixed;
		top: -80px;
		left: 200px;
		background: var(--card);
		padding: 12px;
		z-index: 100;
	}
	.skip:focus {
		top: 10px;
	}
	#content:focus {
		outline: none;
	}
	@media (max-width: 760px) {
		.app-shell {
			display: block;
		}
		.sidebar {
			position: sticky;
			top: 0;
			width: 100%;
			height: auto;
			padding: 10px 14px;
			flex-direction: row;
			align-items: center;
			gap: 14px;
		}
		.brand {
			font-size: 22px;
			padding: 0;
		}
		nav {
			display: flex;
			flex: 1;
			justify-content: flex-end;
			gap: 1px;
		}
		nav a {
			padding: 8px;
			font-size: 11px;
			gap: 6px;
		}
		nav a span {
			display: none;
		}
		.theme {
			margin: 0;
		}
		.workspace {
			margin-left: 0;
		}
		.skip {
			left: 10px;
		}
	}
	.setup-overlay {
		position: fixed;
		inset: 0;
		z-index: 80;
		display: grid;
		place-items: center;
		padding: 24px;
		background: color-mix(in srgb, var(--bg) 72%, transparent);
		backdrop-filter: blur(6px);
	}
	.setup-card {
		width: min(420px, 100%);
		text-align: center;
		padding: 32px 28px;
	}
	.setup-icon {
		display: inline-flex;
		color: var(--accent);
		margin-bottom: 14px;
	}
	.setup-card h2 {
		font-size: 18px;
		margin-bottom: 8px;
	}
	.setup-card > p {
		font-size: 13px;
		white-space: pre-line;
		overflow-wrap: anywhere;
		margin-bottom: 18px;
	}
	.setup-bar {
		height: 6px;
		border-radius: 99px;
		background: var(--line);
		overflow: hidden;
		margin-bottom: 10px;
	}
	.setup-fill {
		height: 100%;
		background: var(--accent);
		border-radius: inherit;
		transition: width 0.2s var(--ease);
	}
	.setup-fill.indeterminate {
		width: 36%;
		animation: setup-slide 1.1s var(--ease) infinite;
	}
	@keyframes setup-slide {
		0% {
			transform: translateX(-120%);
		}
		100% {
			transform: translateX(320%);
		}
	}
	.setup-pct {
		display: block;
		font-size: 11px;
		color: var(--muted);
		margin-bottom: 14px;
	}
	.setup-note {
		font-size: 11px !important;
		color: var(--muted);
		margin: 18px 0 0 !important;
	}
	.setup-card .primary {
		margin-top: 4px;
	}
</style>
