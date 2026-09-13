<script lang="ts">
	let { name, size = 20 }: { name: string; size?: number } = $props();
	const paths: Record<string, string> = {
		search: 'm21 21-4.5-4.5 M19 10.5a8.5 8.5 0 1 1-17 0 8.5 8.5 0 0 1 17 0',
		book: 'M12 6c-3-3-7-3-10-2v15c4-1 7 0 10 2 3-2 6-3 10-2V4c-3-1-7-1-10 2Zm0 0v15',
		bookmark: 'M6 3h12v18l-6-4-6 4V3Z',
		review: 'M3 10a9 9 0 1 1 2 8 M3 3v7h7 M12 7v5l3 2',
		chart: 'M4 3v18h17 M9 16v-5 M14 16V7 M19 16V3',
		settings: 'M4 7h16 M4 17h16 M9 4v6 M15 14v6',
		arrow: 'M4 12h16 m-6-6 6 6-6 6',
		check: 'm5 12 4 4L19 6',
		close: 'm6 6 12 12 M6 18 18 6',
		sun: 'M12 2v2 M12 20v2 M2 12h2 M20 12h2 M5 5l1 1 M18 18l1 1 M5 19l1-1 M18 6l1-1 M16 12a4 4 0 1 1-8 0 4 4 0 0 1 8 0',
		moon: 'M20 15A9 9 0 0 1 9 4a9 9 0 1 0 11 11Z',
		sound: 'M11 4 5 9H2v6h3l6 5V4Zm4 4c2 2 2 6 0 8 M18 5c4 4 4 10 0 14',
		copy: 'M8 8h12v13H8V8Z M16 8V3H3v13h5',
		back: 'M20 12H4m6-6-6 6 6 6',
		download: 'M12 3v12m-5-5 5 5 5-5 M4 16v5h16v-5',
		spark: 'm12 3 2.5 6.5L21 12l-6.5 2.5L12 21l-2.5-6.5L3 12l6.5-2.5L12 3Z'
	};
</script>

<svg
	width={size}
	height={size}
	viewBox="0 0 24 24"
	fill="none"
	stroke="currentColor"
	stroke-width="1.7"
	stroke-linecap="round"
	stroke-linejoin="round"
	aria-hidden="true"><path d={paths[name] ?? paths.book} /></svg
>
