<script lang="ts">
	import '../app.css';
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import Icon from '$lib/components/Icon.svelte';
	import { isPreview } from '$lib/api';
	import { preferences, loadPreferences, applyPreferences } from '$lib/preferences.svelte';
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
</style>
