import { test, expect, type Page } from '@playwright/test';
import seed from '../src-tauri/src/dictionary/seed/seed.json' with { type: 'json' };

const entries = seed.words.map((w, i) => ({
	id: i + 1,
	word: w.word,
	display: w.word,
	phonetic: w.phonetic,
	frequency_rank: w.frequency_rank,
	senses: w.senses.map((s, j) => ({
		id: i * 10 + j,
		pos: s.pos,
		english_definition: s.english,
		chinese_definition: s.chinese,
		level: s.level,
		examples: s.examples.map((ex, k) => ({ ...ex, id: i * 100 + j * 10 + k }))
	})),
	collocations: w.collocations,
	synonyms: w.synonyms,
	antonyms: w.antonyms,
	word_family: w.word_family
}));

/** Blanks every standalone occurrence of `word` (mirrors the Rust grader output shape). */
function blank(text: string, word: string): string | null {
	const re = new RegExp(`\\b${word}\\b`, 'gi');
	return re.test(text) ? text.replace(re, '______') : null;
}

// UI contract fixture only. SQLite migrations and business rules are tested by cargo test.
async function desktopFixture(page: Page) {
	await page.addInitScript((entries) => {
		const blank = (text: string, word: string): string | null => {
			const re = new RegExp(`\\b${word}\\b`, 'gi');
			return re.test(text) ? text.replace(re, '______') : null;
		};
		const win = window as any;
		win.isTauri = true;
		win.__calls = [];
		win.__aiNetwork = 0;
		win.__vt = [];
		const origStartViewTransition = document.startViewTransition?.bind(document);
		if (origStartViewTransition) {
			document.startViewTransition = ((cb: Parameters<Document['startViewTransition']>[0]) => {
				win.__vt.push(document.documentElement.dataset.revealing ?? null);
				return origStartViewTransition(cb);
			}) as Document['startViewTransition'];
		}
		win.__TAURI_INTERNALS__ = {
			invoke: async (command: string, args: any = {}) => {
				win.__calls.push({ command, args });
				const notes = JSON.parse(localStorage.getItem('test.notes') ?? '{}');
				const visited = JSON.parse(localStorage.getItem('test.visited') ?? '[]');
				const save = (key: string, value: any) => localStorage.setItem(key, JSON.stringify(value));
				const flag = (key: string) => localStorage.getItem(key) === '1';
				const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
				const reviewIds = (): number[] => {
					const raw = localStorage.getItem('test.review.ids');
					return raw ? (JSON.parse(raw) as number[]) : [1, 3];
				};
				const defOnly = new Set(JSON.parse(localStorage.getItem('test.review.defonly') ?? '[]'));
				switch (command) {
					case 'app_info':
						return { word_count: 50, schema_version: 4, fts_ok: true, dictionary_ready: true };
					case 'stats_summary':
						return {
							encountered: visited.length,
							learning: visited.length,
							familiar: 0,
							stable: 0,
							ignored: 0,
							due_now: 2,
							high_priority: 0
						};
					case 'recent_words':
						return visited
							.slice(-6)
							.reverse()
							.map((id: number) => ({
								...entries[id - 1],
								visited_at: new Date().toISOString(),
								comprehension_level: 'unknown'
							}));
					case 'search_suggest':
						await wait(args.query === 'm' ? 450 : 25);
						return entries.filter((e) => e.word.startsWith(args.query)).slice(0, 8);
					case 'lookup_word': {
						let entry = entries.find((e) => e.word === args.word);
						if (entry && flag('test.lookup.noexamples')) {
							entry = {
								...entry,
								senses: entry.senses.map((sense) => ({ ...sense, examples: [] }))
							};
						}
						if (entry && !visited.includes(entry.id)) {
							visited.push(entry.id);
							save('test.visited', visited);
						}
						return {
							outcome: entry ? { type: 'hit', entry } : { type: 'miss', suggestions: [] },
							encounter_id: entry?.id ?? null,
							memory: entry ? { status: 'learning', visit_count: 1, high_priority: false } : null,
							comprehension_level: 'unknown'
						};
					}
					case 'word_note':
						return notes[args.wordId] ?? { bookmarked: false, note: '', ignored: false };
					case 'save_word_note':
						if (win.__saveFails) throw { code: 'db', message: '测试：笔记保存失败，请重试' };
						await wait(100);
						notes[args.wordId] = {
							...notes[args.wordId],
							bookmarked: args.bookmarked,
							note: args.note
						};
						save('test.notes', notes);
						return null;
					case 'set_word_ignored':
						notes[args.wordId] = { ...notes[args.wordId], ignored: args.ignored };
						save('test.notes', notes);
						return null;
					case 'set_comprehension':
						return null;
					case 'library_words': {
						let words = entries
							.filter((e) => visited.includes(e.id))
							.map((e) => ({
								...e,
								definition: e.senses[0].chinese_definition,
								status: notes[e.id]?.ignored ? 'ignored' : 'learning',
								bookmarked: notes[e.id]?.bookmarked ?? false,
								note: notes[e.id]?.note ?? '',
								visit_count: 1,
								next_review_at: null
							}));
						words = words.filter(
							(w) =>
								(args.filter === 'all' ||
									(args.filter === 'bookmarked' && w.bookmarked) ||
									args.filter === w.status) &&
								(!args.query || w.word.includes(args.query) || w.note.includes(args.query))
						);
						return { total: words.length, words: words.slice(args.offset, args.offset + 24) };
					}
					case 'review_group': {
						const ids = reviewIds();
						const out = [];
						for (const id of ids.slice(0, 10)) {
							const original = entries[id - 1];
							if (!original) continue;
							const entry = defOnly.has(id)
								? { ...original, senses: original.senses.map((sense) => ({ ...sense, examples: [] })) }
								: original;
							const sense = entry.senses.find((sense) => sense.chinese_definition?.trim()) ??
								entry.senses[0];
							const item = {
								word_id: entry.id,
								prompt: sense.chinese_definition ?? sense.english_definition,
								kind: sense.chinese_definition ? 'definition_zh' : 'definition',
								source: 'dictionary',
								sense_id: sense.id,
								example_id: null
							};
							out.push({ item, entry });
						}
						return out;
					}
					case 'start_review_item':
						await wait(80);
						return args.wordId;
					case 'request_hint':
					case 'practice_hint':
						await wait(150);
						return {
							kind: ['mask', 'english', 'chinese'][args.hintNo - 1],
							text: ['m_________', 'very careful about details', '一丝不苟的'][args.hintNo - 1]
						};
					case 'submit_review': {
						await wait(220);
						const correct = args.answer === entries[args.reviewId - 1].word;
						return {
							correct,
							quality: correct ? 'excellent' : 'forgotten',
							answer: entries[args.reviewId - 1].word,
							hints_used: 0,
							memory: { next_review_at: new Date(Date.now() + 86400000).toISOString() }
						};
					}
					case 'practice_check':
						return {
							correct:
								String(args.answer ?? '').trim().toLowerCase() ===
								entries[args.wordId - 1].word
						};
					case 'ai_config_get':
						return {
							enabled: flag('test.ai.enabled'),
							base_url:
								localStorage.getItem('test.ai.base_url') ?? 'https://api.deepseek.com/v1',
							model: localStorage.getItem('test.ai.model') ?? 'deepseek-chat',
							has_key: flag('test.ai.has_key')
						};
					case 'ai_config_save': {
						localStorage.setItem('test.ai.enabled', args.enabled ? '1' : '0');
						localStorage.setItem('test.ai.base_url', args.baseUrl);
						localStorage.setItem('test.ai.model', args.model);
						if (args.key === '') localStorage.removeItem('test.ai.has_key');
						else if (args.key) localStorage.setItem('test.ai.has_key', '1');
						return {
							enabled: args.enabled,
							base_url: args.baseUrl,
							model: args.model,
							has_key: flag('test.ai.has_key')
						};
					}
					case 'ai_test_connection':
						if (localStorage.getItem('test.ai.mode') === 'fail')
							throw { code: 'network', message: 'mock: 401 Unauthorized' };
						return null;
					case 'ai_generate_examples': {
						const items: Array<{ word_id: number; sense_id: number | null }> = args.items;
						if (!flag('test.ai.enabled')) return [];
						const cache = JSON.parse(localStorage.getItem('test.ai.cache') ?? '{}');
						const missing = items.filter((i) => cache[i.word_id] === undefined);
						if (missing.length) {
							// Counter survives same-tab navigation (a new document resets window).
							sessionStorage.setItem(
								'test.aiNetwork',
								String(Number(sessionStorage.getItem('test.aiNetwork') ?? 0) + 1)
							);
							const mode = localStorage.getItem('test.ai.mode') ?? 'success';
							if (mode === 'fail') throw { code: 'network', message: 'mock: connection refused' };
							if (mode === 'hang') return new Promise(() => {});
							if (mode !== 'invalid') {
								for (const i of missing) {
									const w = entries[i.word_id - 1];
									const sentence = `The result was clearly ${w.word} in the final data.`;
									cache[i.word_id] = blank(sentence, w.word);
								}
								localStorage.setItem('test.ai.cache', JSON.stringify(cache));
							}
						}
						return items
							.filter((i) => cache[i.word_id])
							.map((i) => ({ word_id: i.word_id, sense_id: i.sense_id, sentence: cache[i.word_id] }));
					}
					case 'fading_words':
						return [];
					case 'activity_days':
						return [];
					default:
						throw Error(`Missing test fixture: ${command}`);
				}
			}
		};
	}, entries);
}

test('read-only preview: keyboard suggestions, full definition, and back navigation', async ({
	page
}) => {
	await page.goto('/');
	await expect(page.getByText('50 entries · offline')).toBeVisible();
	await page.screenshot({ path: 'docs/screenshots/home-light.png', fullPage: true });
	const input = page.getByRole('combobox', { name: 'Search for a word' });
	await input.fill('met');
	await expect(page.getByRole('option', { name: /meticulous/ })).toBeVisible();
	await input.press('ArrowDown');
	await input.press('Enter');
	await expect(page.getByRole('heading', { name: 'meticulous', exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'Bookmark this word' })).toBeDisabled();
	await page.getByRole('button', { name: 'Full entry', exact: true }).click();
	await expect(page.getByText('一丝不苟的；极其仔细的', { exact: true })).toBeVisible();
	await page.screenshot({ path: 'docs/screenshots/word-light.png', fullPage: true });
	await page.locator('a.back').click();
	await expect(page.getByRole('combobox', { name: 'Search for a word' })).toBeVisible();
});

test('lookup generates one readable AI sentence only when dictionary examples are absent', async ({ page }) => {
	await desktopFixture(page);
	await page.goto('/');
	await page.evaluate(() => {
		localStorage.setItem('test.ai.enabled', '1');
		localStorage.setItem('test.ai.has_key', '1');
	});
	await page.goto('/?word=meticulous');
	await expect(page.getByText('She was meticulous about checking every detail before submitting the report.')).toBeVisible();
	await expect.poll(() => page.evaluate(() => (window as any).__calls.filter((call: any) => call.command === 'ai_generate_examples').length)).toBe(0);

	await page.evaluate(() => localStorage.setItem('test.lookup.noexamples', '1'));
	await page.reload();
	await expect(page.getByText('The result was clearly meticulous in the final data.')).toBeVisible();
	await expect(page.getByText('AI example')).toBeVisible();
	await expect.poll(() => page.evaluate(() => (window as any).__calls.filter((call: any) => call.command === 'ai_generate_examples').length)).toBe(1);
});

test('AI example shows a waiting animation while generation is pending', async ({ page }) => {
	await desktopFixture(page);
	await page.goto('/');
	await page.evaluate(() => {
		localStorage.setItem('test.lookup.noexamples', '1');
		localStorage.setItem('test.ai.enabled', '1');
		localStorage.setItem('test.ai.mode', 'hang');
	});
	await page.goto('/?word=meticulous');
	const waiting = page.locator('.ai-wait');
	await expect(waiting).toBeVisible();
	await expect(waiting).toContainText('Writing an example sentence…');
	await expect(waiting.locator('.spinner')).toHaveCSS('animation-name', /spin$/);
	await page.screenshot({ path: 'test-results/ai-example-waiting.png' });
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await expect(waiting.locator('.spinner')).toHaveCSS('animation-name', 'none');
});

test('missing words expose recovery and dictionary management', async ({ page }) => {
	await page.goto('/?word=zzzzzzzz');
	await expect(page.getByRole('heading', { name: /No entry for “zzzzzzzz”/ })).toBeVisible();
	await page.getByRole('link', { name: 'Settings', exact: true }).click();
	await expect(page.locator('#source-path')).toBeDisabled();
});

test('reading mode and theme persist after reload', async ({ page }) => {
	await page.goto('/settings');
	await page.getByRole('button', { name: 'Dark', exact: true }).click();
	await page.getByRole('button', { name: 'Full entry', exact: true }).click();
	await page.getByRole('button', { name: 'Large', exact: true }).click();
	await page.reload();
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
	await expect(page.getByRole('button', { name: 'Full entry', exact: true })).toHaveAttribute(
		'aria-pressed',
		'true'
	);
	await page.goto('/?word=meticulous');
	await expect(page.getByRole('heading', { name: 'Definitions' })).toBeVisible();
	await page.screenshot({ path: 'docs/screenshots/word-dark.png', fullPage: true });
});

test('sidebar collapses to icons and persists on desktop', async ({ page }) => {
	await desktopFixture(page);
	await page.goto('/');
	const sidebar = page.locator('aside.sidebar');
	const icon = sidebar.getByRole('link', { name: 'Search', exact: true }).locator('svg');
	const iconBefore = await icon.boundingBox();
	const toggleBefore = await page.getByRole('button', { name: 'Collapse sidebar' }).boundingBox();
	await page.getByRole('button', { name: 'Collapse sidebar' }).click();
	await expect(sidebar).toHaveClass(/collapsed/);
	await expect(page.getByRole('button', { name: 'Expand sidebar' })).toBeVisible();
	await expect(sidebar.getByText('Search', { exact: true })).toBeHidden();
	await expect.poll(() => sidebar.locator('.brand-middle').evaluate(el => el.getBoundingClientRect().width)).toBe(0);
	await expect(sidebar.locator('.brand-dot')).toBeVisible();
	await expect(sidebar.getByRole('link', { name: 'Lexica home' })).toBeVisible();
	await expect
		.poll(() => page.evaluate(() => getComputedStyle(document.querySelector('.workspace')!).marginLeft))
		.toBe('68px');
	const iconAfter = await icon.boundingBox();
	expect(iconAfter!.x).toBeCloseTo(iconBefore!.x, 0);
	expect(iconAfter!.y).toBeCloseTo(iconBefore!.y, 0);
	const toggleAfter = await page.getByRole('button', { name: 'Expand sidebar' }).boundingBox();
	expect(toggleAfter!.y).toBe(toggleBefore!.y);
	expect(toggleAfter!.x + toggleAfter!.width / 2).toBeCloseTo(67, 0);
	await page.screenshot({ path: 'test-results/sidebar-collapsed.png' });
	await page.reload();
	await expect(sidebar).toHaveClass(/collapsed/);
	await page.getByRole('button', { name: 'Expand sidebar' }).focus();
	await page.keyboard.press('Enter');
	await expect(sidebar).not.toHaveClass(/collapsed/);
	await expect.poll(() => sidebar.locator('.brand-middle').evaluate(el => getComputedStyle(el).opacity)).toBe('1');
	await page.screenshot({ path: 'test-results/sidebar-expanded.png' });
});

for (const width of [480, 1180])
	test(`all pages fit a ${width}px viewport`, async ({ page }) => {
		await page.setViewportSize({ width, height: 820 });
		for (const route of ['/', '/?word=meticulous', '/library', '/review', '/stats', '/settings']) {
			await page.goto(route);
			await expect(page.locator('main')).toBeVisible();
			await expect
				.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
				.toBe(true);
			// Landmark per route: the home page has no h1 (search-first layout);
			// the review page in its empty preview state shows the empty state heading.
			if (route === '/') await expect(page.getByRole('combobox', { name: 'Search for a word' })).toBeVisible();
			else if (route === '/review') await expect(page.getByRole('heading', { name: 'Nothing due' })).toBeVisible();
			else await expect(page.locator('h1')).toBeVisible();
		}
		if (width === 480)
			await page.screenshot({ path: 'docs/screenshots/settings-mobile.png', fullPage: true });
	});

test('desktop contract: bookmark, save notes, reload, filter, pause and resume', async ({
	page
}) => {
	await desktopFixture(page);
	await page.goto('/?word=meticulous');
	const bookmark = page.getByRole('button', { name: 'Bookmark this word' });
	await expect(bookmark).toBeEnabled();
	await bookmark.click();
	await expect(page.getByRole('button', { name: 'Remove bookmark' })).toHaveAttribute(
		'aria-pressed',
		'true'
	);
	await page.getByLabel('Word notes').fill('联想到检查每一个细节。');
	await page.getByRole('button', { name: 'Save', exact: true }).click();
	// The word stays bookmarked, so the save confirmation reads "Bookmarked".
	await expect(page.getByRole('status').filter({ hasText: 'Bookmarked' })).toBeVisible();
	await page.reload();
	await expect(page.getByLabel('Word notes')).toHaveValue('联想到检查每一个细节。');
	await page.getByRole('button', { name: 'Pause reminders' }).click();
	await expect(page.getByRole('button', { name: 'Resume reminders' })).toBeVisible();
	await page.getByRole('link', { name: 'Words', exact: true }).click();
	await page.getByRole('button', { name: 'Saved', exact: true }).click();
	await expect(page.getByText('meticulous', { exact: true })).toBeVisible();
	await page.getByLabel('Search words and notes').fill('每一个细节');
	await expect(page.getByText('meticulous', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'Resume', exact: true }).click();
	await expect(page.getByRole('button', { name: 'Resume', exact: true })).toHaveCount(0);
});

test('desktop contract: failed saves retain drafts and show readable errors', async ({ page }) => {
	await desktopFixture(page);
	await page.goto('/?word=meticulous');
	await expect(page.getByLabel('Word notes')).toBeEnabled();
	await page.getByLabel('Word notes').fill('不能丢失的笔记');
	await page.getByRole('combobox', { name: 'Search for a word' }).press('Enter');
	await expect(page.getByLabel('Word notes')).toHaveValue('不能丢失的笔记');
	await page.evaluate(() => {
		(window as any).__saveFails = true;
	});
	await page.getByRole('button', { name: 'Save', exact: true }).click();
	await expect(page.getByRole('alert')).toContainText('笔记保存失败');
	await expect(page.getByLabel('Word notes')).toHaveValue('不能丢失的笔记');
	await page.evaluate(() => {
		(window as any).__saveFails = false;
	});
	await page.getByRole('button', { name: 'Save', exact: true }).click();
	await expect(page.getByRole('status').filter({ hasText: 'Saved' })).toBeVisible();
});

function fixtureAnswer(prompt: string, _defOnlyIds: number[]): string {
	for (const e of entries) {
		for (const s of e.senses) {
			if (
				s.chinese_definition === prompt ||
				s.english_definition === prompt ||
				blank(s.english_definition, e.word) === prompt
			)
				return e.word;
			for (const ex of s.examples) {
				if (ex.text.replace(new RegExp(`\\b${e.word}\\b`, 'gi'), '______') === prompt) {
					return e.word;
				}
			}
		}
	}
	throw new Error(`No fixture word for prompt: ${prompt}`);
}

async function finishBrowsingAndStart(page: Page) {
	await expect(page.getByText(/Browse · /)).toBeVisible();
	const progress = await page.getByText(/Browse · /).innerText();
	const total = Number(progress.match(/\/\s*(\d+)/)?.[1] ?? 1);
	for (let i = 1; i < total; i++) {
		await page.getByRole('button', { name: 'Next', exact: true }).click();
	}
	await page.getByRole('button', { name: 'Start practice' }).click();
	await expect(page.getByLabel('Which word?')).toBeVisible();
}

async function browseAndStart(page: Page) {
	await page.goto('/review');
	await finishBrowsingAndStart(page);
}

test('desktop contract: browse phase is read-only, group shuffles, all words asked once', async ({
	page
}) => {
	await desktopFixture(page);
	// localStorage needs a real origin: land on the app before configuring the fixture.
	await page.goto('/');
	await page.evaluate(() =>
		localStorage.setItem('test.review.ids', JSON.stringify([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]))
	);
	await page.goto('/review');
	// Group is capped at 10 even when 12 words are due.
	await expect(page.getByText(/Browse · 1 \/ 10/)).toBeVisible();
	// Browse phase records nothing: no lookup_word, no set_comprehension, no pending reviews.
	let calls = await page.evaluate(() => (window as any).__calls.map((c: any) => c.command));
	expect(calls).not.toContain('lookup_word');
	expect(calls).not.toContain('set_comprehension');
	expect(calls).not.toContain('start_review_item');
	await expect(page.getByRole('button', { name: 'Previous' })).toBeDisabled();
	await page.getByRole('button', { name: 'Next', exact: true }).click();
	await expect(page.getByText(/Browse · 2 \/ 10/)).toBeVisible();
	await page.getByRole('button', { name: 'Next', exact: true }).click();
	await expect(page.getByText(/Browse · 3 \/ 10/)).toBeVisible();
	await page.getByRole('button', { name: 'Previous' }).click();
	await expect(page.getByText(/Browse · 2 \/ 10/)).toBeVisible();
	await expect(page.getByRole('button', { name: 'Start practice' })).toBeDisabled();
	for (let i = 2; i < 10; i++) {
		await page.getByRole('button', { name: 'Next', exact: true }).click();
	}
	await expect(page.getByText(/Browse · 10 \/ 10/)).toBeVisible();
	await page.getByRole('button', { name: 'Start practice' }).click();
	await expect(page.getByLabel('Which word?')).toBeVisible();

	// Practice asks every group word exactly once (set coverage, order shuffled).
	const asked = new Set<number>();
	for (let i = 0; i < 10; i++) {
		const prompt = await page.locator('.prompt').innerText();
		const word = fixtureAnswer(prompt, []);
		expect(asked.has(entries.findIndex((e) => e.word === word))).toBe(false);
		asked.add(entries.findIndex((e) => e.word === word));
		await page.getByLabel('Which word?').fill(word);
		await page.getByRole('button', { name: 'Check' }).click();
		await expect(page.getByRole('status').filter({ hasText: word })).toBeVisible();
		await page.getByRole('button', { name: /Next|Finish/ }).click();
		if (i < 9) await expect(page.getByLabel('Which word?')).toBeVisible();
	}
	expect(asked.size).toBe(10);
	await expect(page.getByRole('heading', { name: 'Group complete' })).toBeVisible();
	await expect(page.getByText('10', { exact: true }).first()).toBeVisible();
	calls = await page.evaluate(() => (window as any).__calls.map((c: any) => c.command));
	expect(calls.filter((c: string) => c === 'submit_review').length).toBe(10);
	expect(calls).not.toContain('lookup_word');
});

test('desktop contract: review prevents double submit and records formal attempts once', async ({
	page
}) => {
	await desktopFixture(page);
	await browseAndStart(page);
	const prompt = await page.locator('.prompt').innerText();
	const word = fixtureAnswer(prompt, []);
	await page.getByLabel('Which word?').fill(word);
	await page.getByRole('button', { name: 'Check' }).click();
	await expect(page.getByRole('status').filter({ hasText: word })).toBeVisible();
	const submissions = await page.evaluate(
		() => (window as any).__calls.filter((c: any) => c.command === 'submit_review').length
	);
	expect(submissions).toBe(1);
	await page.getByRole('button', { name: /Next|Finish/ }).click();
	await expect(page.getByLabel('Which word?')).toBeVisible();
	// Reveal on a first attempt is the formal Forgotten record (retry text comes later).
	await page.getByRole('button', { name: 'Reveal' }).click();
	await expect(page.getByText('Forgotten')).toBeVisible();
	await page.getByRole('button', { name: /Next|Finish/ }).click();
	await expect(page.getByLabel('Which word?')).toBeVisible();
});

test('desktop contract: wrong words re-queue and retries never double-count formal records', async ({
	page
}) => {
	await desktopFixture(page);
	await page.goto('/');
	await page.evaluate(() => localStorage.setItem('test.review.ids', JSON.stringify([1])));
	await page.goto('/review');
	await expect(page.getByText(/Browse · 1 \/ 1/)).toBeVisible();
	await page.getByRole('button', { name: 'Start practice' }).click();
	await expect(page.getByLabel('Which word?')).toBeVisible();

	// First (formal) attempt: wrong answer → re-queued to the tail.
	await page.getByLabel('Which word?').fill('wrongword');
	await page.getByRole('button', { name: 'Check' }).click();
	await expect(page.getByText('Forgotten')).toBeVisible();
	await expect(page.getByText('1 to revisit')).toBeVisible();
	await page.getByRole('button', { name: /Next|Finish/ }).click();
	await expect(page.getByLabel('Which word?')).toBeVisible();

	// Retry via reveal → re-queued again, still read-only.
	await page.getByRole('button', { name: 'Reveal' }).click();
	await expect(page.getByText('Not quite — this word will come back')).toBeVisible();
	await page.getByRole('button', { name: /Next|Finish/ }).click();
	await expect(page.getByLabel('Which word?')).toBeVisible();

	// Retry answered correctly → read-only grading, session completes.
	await page.getByLabel('Which word?').fill('meticulous');
	await page.getByRole('button', { name: 'Check' }).click();
	await expect(page.getByText('Correct — recalled on retry')).toBeVisible();
	await expect(page.getByText('1 / 1 recalled')).toBeVisible();
	await expect(page.getByText('0 to revisit')).toBeVisible();
	await page.getByRole('button', { name: /Next|Finish/ }).click();
	await expect(page.getByRole('heading', { name: 'Group complete' })).toBeVisible();
	await expect(page.getByText('0', { exact: true }).first()).toBeVisible();
	await expect(page.getByText('1', { exact: true }).first()).toBeVisible();

	const calls = await page.evaluate(() => (window as any).__calls.map((c: any) => c.command));
	// Exactly one formal record despite three attempts; retries used the read-only path.
	expect(calls.filter((c: string) => c === 'submit_review').length).toBe(1);
	expect(calls.filter((c: string) => c === 'practice_check').length).toBe(2);
	expect(calls.filter((c: string) => c === 'start_review_item').length).toBe(1);
});

test('desktop contract: retrying a failed hint preserves the formal review and prior hints', async ({
	page
}) => {
	await desktopFixture(page);
	await page.goto('/');
	await page.evaluate(() => localStorage.setItem('test.review.ids', JSON.stringify([1])));
	await page.goto('/review');
	await finishBrowsingAndStart(page);
	await page.getByRole('button', { name: /First letter/ }).click();
	await expect(page.getByText('m_________', { exact: true })).toBeVisible();
	await page.evaluate(() => {
		const win = window as any;
		const invoke = win.__TAURI_INTERNALS__.invoke;
		let failOnce = true;
		win.__TAURI_INTERNALS__.invoke = async (command: string, args: unknown) => {
			if (command === 'request_hint' && failOnce) {
				failOnce = false;
				throw new Error('temporary hint failure');
			}
			return invoke(command, args);
		};
	});
	await page.getByRole('button', { name: /English definition/ }).click();
	await expect(page.getByRole('alert')).toContainText('temporary hint failure');
	await page.getByRole('button', { name: 'Retry', exact: true }).click();
	await expect(page.getByText('m_________', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: /English definition/ })).toBeEnabled();
	const starts = await page.evaluate(
		() => (window as any).__calls.filter((c: any) => c.command === 'start_review_item').length
	);
	expect(starts).toBe(1);
});

test('desktop contract: ordered hints never reveal the next hint early', async ({ page }) => {
	await desktopFixture(page);
	await page.goto('/');
	await page.evaluate(() => localStorage.setItem('test.review.ids', JSON.stringify([1])));
	await page.goto('/review');
	await page.getByRole('button', { name: 'Start practice' }).click();
	await expect(page.getByLabel('Which word?')).toBeVisible();
	await page.getByRole('button', { name: /First letter/ }).click();
	await expect(page.getByText('m_________', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: /English definition/ }).click();
	await expect(page.getByText('very careful about details', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: /Chinese definition/ }).click();
	await expect(page.getByText('一丝不苟的', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: /No hints left/ })).toBeDisabled();
	await page.screenshot({ path: 'docs/screenshots/review-fixture.png', fullPage: true });
});

test('desktop contract: slow suggestion responses cannot replace newer input', async ({ page }) => {
	await desktopFixture(page);
	await page.goto('/');
	const input = page.getByRole('combobox', { name: 'Search for a word' });
	await input.fill('m');
	await expect
		.poll(() =>
			page.evaluate(() =>
				(window as any).__calls.some(
					(c: any) => c.command === 'search_suggest' && c.args.query === 'm'
				)
			)
		)
		.toBe(true);
	await input.fill('sub');
	await expect(page.getByRole('option', { name: /subtle/ })).toBeVisible();
	await expect
		.poll(() => page.getByRole('option', { name: /meticulous/ }).count(), { timeout: 1000 })
		.toBe(0);
	await page.waitForTimeout(500);
	await expect(page.getByRole('option', { name: /subtle/ })).toBeVisible();
	await input.press('Escape');
	await expect(page.getByRole('listbox')).toHaveCount(0);
});

// ── Theme animation ─────────────────────────────────────────

test('theme expand: dark → light grows from the click point', async ({ page }) => {
	await desktopFixture(page);
	await page.goto('/settings');
	await page.getByRole('button', { name: 'Dark', exact: true }).click();
	const toggle = page.locator('button.theme');
	await toggle.click({ position: { x: 4, y: 4 } });
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
	const vt = await page.evaluate(() => (window as any).__vt);
	expect(vt).toEqual(['expand']);
	// Circle origin is the pointer position (4,4 inside the button), not the centre.
	const box = await toggle.boundingBox();
	expect(box).not.toBeNull();
	const vars = await page.evaluate(() => ({
		x: document.documentElement.style.getPropertyValue('--reveal-x'),
		y: document.documentElement.style.getPropertyValue('--reveal-y')
	}));
	expect(Math.abs(parseFloat(vars.x) - (box!.x + 4))).toBeLessThan(1);
	expect(Math.abs(parseFloat(vars.y) - (box!.y + 4))).toBeLessThan(1);
	await expect
		.poll(() => page.evaluate(() => document.documentElement.dataset.revealing ?? null))
		.toBe(null);
});

test('theme shrink: light → dark contracts into the click point', async ({ page }) => {
	await desktopFixture(page);
	await page.goto('/settings');
	const toggle = page.locator('button.theme');
	await toggle.click({ position: { x: 30, y: 20 } });
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
	const vt = await page.evaluate(() => (window as any).__vt);
	expect(vt).toEqual(['shrink']);
	const box = await toggle.boundingBox();
	const vars = await page.evaluate(() => ({
		x: document.documentElement.style.getPropertyValue('--reveal-x'),
		y: document.documentElement.style.getPropertyValue('--reveal-y')
	}));
	expect(Math.abs(parseFloat(vars.x) - (box!.x + 30))).toBeLessThan(1);
	expect(Math.abs(parseFloat(vars.y) - (box!.y + 20))).toBeLessThan(1);
	await expect
		.poll(() => page.evaluate(() => document.documentElement.dataset.revealing ?? null))
		.toBe(null);
});

test('theme via keyboard uses the button centre as the circle origin', async ({ page }) => {
	await desktopFixture(page);
	await page.goto('/settings');
	const toggle = page.locator('button.theme');
	await toggle.focus();
	await page.keyboard.press('Enter');
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
	const vt = await page.evaluate(() => (window as any).__vt);
	expect(vt).toEqual(['shrink']);
	const box = await toggle.boundingBox();
	const vars = await page.evaluate(() => ({
		x: document.documentElement.style.getPropertyValue('--reveal-x'),
		y: document.documentElement.style.getPropertyValue('--reveal-y')
	}));
	expect(Math.abs(parseFloat(vars.x) - (box!.x + box!.width / 2))).toBeLessThan(1);
	expect(Math.abs(parseFloat(vars.y) - (box!.y + box!.height / 2))).toBeLessThan(1);
});

test('theme rapid clicks keep only the newest transition and clean up', async ({ page }) => {
	await desktopFixture(page);
	await page.goto('/settings');
	const toggle = page.locator('button.theme');
	await toggle.click();
	await toggle.click();
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
	const vt = await page.evaluate(() => (window as any).__vt);
	expect(vt).toEqual(['shrink', 'expand']);
	await expect
		.poll(() => page.evaluate(() => document.documentElement.dataset.revealing ?? null))
		.toBe(null);
	// Still animatable afterwards: another click triggers a fresh transition.
	await toggle.click();
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});

test('theme with reduced motion switches directly without a view transition', async ({ page }) => {
	await desktopFixture(page);
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await page.goto('/settings');
	await page.locator('button.theme').click();
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
	const vt = await page.evaluate(() => (window as any).__vt);
	expect(vt).toEqual([]);
});

// ── Review grouping + AI ────────────────────────────────────

test('review group sizes: empty, single word, and fewer than ten', async ({ page }) => {
	await desktopFixture(page);
	await page.goto('/');
	await page.evaluate(() => localStorage.setItem('test.review.ids', JSON.stringify([])));
	await page.goto('/review');
	await expect(page.getByRole('heading', { name: 'Nothing due' })).toBeVisible();

	await page.evaluate(() => localStorage.setItem('test.review.ids', JSON.stringify([1])));
	await page.goto('/review');
	await expect(page.getByText(/Browse · 1 \/ 1/)).toBeVisible();

	await page.evaluate(() => localStorage.setItem('test.review.ids', JSON.stringify([1, 3, 5])));
	await page.goto('/review');
	await expect(page.getByText(/Browse · 1 \/ 3/)).toBeVisible();
});

test('AI off: review uses definitions even when dictionary examples exist', async ({ page }) => {
	await desktopFixture(page);
	await page.goto('/');
	await page.evaluate(() =>
		localStorage.setItem('test.review.ids', JSON.stringify([1, 3]))
	);
	await page.goto('/review');
	await expect(page.getByText(/Browse · 1 \/ 2/)).toBeVisible();
	// No AI request was made.
	await page.waitForTimeout(200);
	let calls = await page.evaluate(() => (window as any).__calls.map((c: any) => c.command));
	expect(calls).not.toContain('ai_generate_examples');
	await expect(page.getByRole('link', { name: 'Configure DeepSeek' })).toHaveAttribute(
		'href',
		'/settings#ai-examples'
	);
	await finishBrowsingAndStart(page);
	await expect(page.locator('.kind')).toHaveText('Chinese definition');
	const word = fixtureAnswer(await page.locator('.prompt').innerText(), []);
	await page.getByLabel('Which word?').fill(word);
	await page.getByRole('button', { name: 'Check' }).click();
	await expect(page.getByRole('status').filter({ hasText: word })).toBeVisible();
	calls = await page.evaluate(() => (window as any).__calls.map((c: any) => c.command));
	expect(calls).toContain('submit_review');
});

test('AI success: review clozes use AI even when the dictionary has examples', async ({
	page
}) => {
	await desktopFixture(page);
	await page.goto('/');
	await page.evaluate(() => localStorage.setItem('test.ai.enabled', '1'));
	await page.evaluate(() => localStorage.setItem('test.ai.has_key', '1'));
	await page.evaluate(() => localStorage.setItem('test.review.ids', JSON.stringify([1])));
	await page.goto('/review');
	await expect(page.getByText('AI examples ready for 1 word.')).toBeVisible();
	await expect(page.locator('.ai-note')).toBeVisible();
	await page.getByRole('button', { name: 'Start practice' }).click();
	// Fixed at practice start: AI cloze, marked as AI-sourced.
	await expect(page.locator('.kind')).toHaveText(/Cloze · AI/);
	await expect(page.locator('.prompt')).toHaveText('The result was clearly ______ in the final data.');
	await page.getByLabel('Which word?').fill('meticulous');
	await page.getByRole('button', { name: 'Check' }).click();
	await expect(page.getByRole('status').filter({ hasText: 'meticulous' })).toBeVisible();
});

test('AI failure and invalid returns fall back to local prompts without blocking', async ({
	page
}) => {
	await desktopFixture(page);
	await page.goto('/');
	await page.evaluate(() => localStorage.setItem('test.ai.enabled', '1'));
	await page.evaluate(() => localStorage.setItem('test.ai.has_key', '1'));
	await page.evaluate(() => localStorage.setItem('test.ai.mode', 'fail'));
	await page.evaluate(() => localStorage.setItem('test.review.ids', JSON.stringify([1])));
	await page.goto('/review');
	await expect(page.locator('.ai-note')).toContainText('AI examples unavailable');
	await page.getByRole('button', { name: 'Start practice' }).click();
	await expect(page.getByLabel('Which word?')).toBeVisible();
	await expect(page.locator('.kind')).toHaveText('Chinese definition');

	// Invalid model output: the fixture (standing in for Rust validation) rejects everything.
	await page.evaluate(() => localStorage.setItem('test.ai.mode', 'invalid'));
	await page.evaluate(() => localStorage.removeItem('test.ai.cache'));
	await page.evaluate(() => localStorage.removeItem('test.ai.has_key'));
	await page.goto('/review');
	await page.getByRole('button', { name: 'Start practice' }).click();
	await expect(page.getByLabel('Which word?')).toBeVisible();
	await expect(page.locator('.kind')).toHaveText('Chinese definition');
});

test('AI generation that has not finished by practice start never blocks it', async ({ page }) => {
	await desktopFixture(page);
	await page.goto('/');
	await page.evaluate(() => localStorage.setItem('test.ai.enabled', '1'));
	await page.evaluate(() => localStorage.setItem('test.ai.has_key', '1'));
	await page.evaluate(() => localStorage.setItem('test.ai.mode', 'hang'));
	await page.evaluate(() => localStorage.setItem('test.review.ids', JSON.stringify([1])));
	await page.goto('/review');
	await expect(page.getByText('Generating AI example sentences…')).toBeVisible();
	// Start immediately — the hanging generation must not block practice.
	await page.getByRole('button', { name: 'Start practice' }).click();
	await expect(page.getByLabel('Which word?')).toBeVisible();
	await expect(page.locator('.kind')).toHaveText('Chinese definition');
});

test('AI examples are cached: a second group does not hit the network again', async ({ page }) => {
	await desktopFixture(page);
	await page.goto('/');
	await page.evaluate(() => localStorage.setItem('test.ai.enabled', '1'));
	await page.evaluate(() => localStorage.setItem('test.ai.has_key', '1'));
	await page.evaluate(() => localStorage.setItem('test.review.ids', JSON.stringify([2])));
	await page.evaluate(() => localStorage.setItem('test.review.defonly', JSON.stringify([2])));
	await page.goto('/review');
	await expect(page.getByText('AI examples ready for 1 word.')).toBeVisible();
	expect(await page.evaluate(() => Number(sessionStorage.getItem('test.aiNetwork') ?? 0))).toBe(1);
	// Same word again (simulating a later group): served from the cache.
	await page.goto('/review');
	await expect(page.getByText('AI examples ready for 1 word.')).toBeVisible();
	expect(await page.evaluate(() => Number(sessionStorage.getItem('test.aiNetwork') ?? 0))).toBe(1);
});

test('AI settings: DeepSeek defaults, save, persistence and connection errors', async ({
	page
}) => {
	await desktopFixture(page);
	await page.goto('/settings');
	await expect(page.getByRole('heading', { name: 'DeepSeek example sentences' })).toBeVisible();
	await page.screenshot({ path: 'docs/screenshots/ai-settings.png', fullPage: true });
	// Off by default.
	await expect(page.getByRole('button', { name: 'Off', exact: true })).toHaveAttribute(
		'aria-pressed',
		'true'
	);
	await page.getByRole('button', { name: 'On', exact: true }).click();
	await expect(page.getByPlaceholder('https://api.deepseek.com/v1')).toHaveValue(
		'https://api.deepseek.com/v1'
	);
	await expect(page.getByPlaceholder('deepseek-chat')).toHaveValue('deepseek-chat');
	await page.getByPlaceholder('https://api.deepseek.com/v1').fill('https://mock.local/v1');
	await page.getByPlaceholder('deepseek-chat').fill('mock-model');
	await page.getByPlaceholder('Paste API key').fill('sk-test');
	await page.getByRole('button', { name: 'Save', exact: true }).click();
	await expect(page.getByText('AI settings saved.')).toBeVisible();
	await expect(page.getByText('· stored')).toBeVisible();
	await page.reload();
	await expect(page.getByPlaceholder('https://api.deepseek.com/v1')).toHaveValue(
		'https://mock.local/v1'
	);
	await expect(page.getByPlaceholder(/Leave empty to keep the stored key/)).toBeVisible();
	// Connection test failure is surfaced verbatim.
	await page.evaluate(() => localStorage.setItem('test.ai.mode', 'fail'));
	await page.getByRole('button', { name: 'Test connection' }).click();
	await expect(page.getByRole('alert')).toContainText('401');
	// Removing the key clears the stored-key state.
	await page.getByRole('button', { name: 'Remove key' }).click();
	await expect(page.getByText('AI settings saved.')).toBeVisible();
	await expect(page.getByText('· stored')).toHaveCount(0);
});
