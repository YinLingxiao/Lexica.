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
		examples: s.examples
	})),
	collocations: w.collocations,
	synonyms: w.synonyms,
	antonyms: w.antonyms,
	word_family: w.word_family
}));

// UI contract fixture only. SQLite migrations and business rules are tested by cargo test.
async function desktopFixture(page: Page) {
	await page.addInitScript((entries) => {
		const win = window as any;
		win.isTauri = true;
		win.__calls = [];
		win.__TAURI_INTERNALS__ = {
			invoke: async (command: string, args: any = {}) => {
				win.__calls.push({ command, args });
				const notes = JSON.parse(localStorage.getItem('test.notes') ?? '{}');
				const visited = JSON.parse(localStorage.getItem('test.visited') ?? '[]');
				const save = (key: string, value: any) => localStorage.setItem(key, JSON.stringify(value));
				const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
				switch (command) {
					case 'app_info':
						return { word_count: 50, schema_version: 3, fts_ok: true };
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
						const entry = entries.find((e) => e.word === args.word);
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
					case 'review_queue':
						return [
							{
								word_id: 1,
								prompt: 'She was ______ about checking every detail.',
								sense_id: 0,
								example_id: 1
							},
							{
								word_id: 2,
								prompt: 'Carefully planned with many details.',
								sense_id: 10,
								example_id: null
							}
						];
					case 'start_review_item':
						await wait(80);
						return args.wordId;
					case 'request_hint':
						await wait(150);
						return {
							kind: ['mask', 'english', 'chinese'][args.hintNo - 1],
							text: ['m_________', 'very careful about details', '一丝不苟的'][args.hintNo - 1]
						};
					case 'submit_review':
						await wait(220);
						return {
							correct: args.answer === entries[args.reviewId - 1].word,
							quality: args.answer ? 'excellent' : 'forgotten',
							answer: entries[args.reviewId - 1].word,
							hints_used: 0,
							memory: { next_review_at: new Date(Date.now() + 86400000).toISOString() }
						};
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
	await expect(page.getByText('50 个本地词条，随时探索')).toBeVisible();
	await page.screenshot({ path: 'docs/screenshots/home-light.png', fullPage: true });
	const input = page.getByRole('combobox', { name: '搜索英文单词' });
	await input.fill('met');
	await expect(page.getByRole('option', { name: /meticulous/ })).toBeVisible();
	await input.press('ArrowDown');
	await input.press('Enter');
	await expect(page.getByRole('heading', { name: 'meticulous', exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: '收藏单词', exact: true })).toBeDisabled();
	await page.getByRole('button', { name: '完整释义', exact: true }).click();
	await expect(page.getByText('一丝不苟的；极其仔细的', { exact: true })).toBeVisible();
	await page.screenshot({ path: 'docs/screenshots/word-light.png', fullPage: true });
	await page.getByRole('link', { name: '返回探索', exact: true }).click();
	await expect(page.getByRole('heading', { name: '每一次遇见，都更懂一点。' })).toBeVisible();
});

test('missing words expose recovery and dictionary management', async ({ page }) => {
	await page.goto('/?word=zzzzzzzz');
	await expect(page.getByRole('heading', { name: '暂未找到「zzzzzzzz」' })).toBeVisible();
	await page.getByRole('link', { name: '管理词库', exact: true }).click();
	await expect(page.getByLabel('词库文件路径')).toBeDisabled();
});

test('reading mode and theme persist after reload', async ({ page }) => {
	await page.goto('/settings');
	await page.getByRole('button', { name: '深色', exact: true }).click();
	await page.getByRole('button', { name: '完整词条', exact: true }).click();
	await page.getByRole('button', { name: '大字', exact: true }).click();
	await page.reload();
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
	await expect(page.getByRole('button', { name: '完整词条', exact: true })).toHaveAttribute(
		'aria-pressed',
		'true'
	);
	await page.goto('/?word=meticulous');
	await expect(page.getByRole('heading', { name: '释义与例句' })).toBeVisible();
	await page.screenshot({ path: 'docs/screenshots/word-dark.png', fullPage: true });
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
			await expect(page.locator('h1')).toBeVisible();
		}
		if (width === 480)
			await page.screenshot({ path: 'docs/screenshots/settings-mobile.png', fullPage: true });
	});

test('desktop contract: bookmark, save notes, reload, filter, pause and resume', async ({
	page
}) => {
	await desktopFixture(page);
	await page.goto('/?word=meticulous');
	const bookmark = page.getByRole('button', { name: '收藏单词', exact: true });
	await expect(bookmark).toBeEnabled();
	await bookmark.click();
	await expect(page.getByRole('button', { name: '取消收藏', exact: true })).toHaveAttribute(
		'aria-pressed',
		'true'
	);
	await page.getByLabel('单词笔记').fill('联想到检查每一个细节。');
	await page.getByRole('button', { name: '保存笔记', exact: true }).click();
	await expect(page.getByText('已保存在本地', { exact: true })).toBeVisible();
	await page.reload();
	await expect(page.getByLabel('单词笔记')).toHaveValue('联想到检查每一个细节。');
	await page.getByRole('button', { name: '暂停复习提醒', exact: true }).click();
	await expect(page.getByRole('button', { name: '恢复复习提醒', exact: true })).toBeVisible();
	await page.getByRole('link', { name: '我的词汇 Vocabulary' }).click();
	await page.getByRole('button', { name: '已收藏', exact: true }).click();
	await expect(page.getByText('meticulous', { exact: true })).toBeVisible();
	await page.getByLabel('搜索我的词汇与笔记').fill('每一个细节');
	await expect(page.getByText('meticulous', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: '恢复提醒', exact: true }).click();
	await expect(page.getByRole('button', { name: '恢复提醒', exact: true })).toHaveCount(0);
});

test('desktop contract: failed saves retain drafts and show readable errors', async ({ page }) => {
	await desktopFixture(page);
	await page.goto('/?word=meticulous');
	await expect(page.getByLabel('单词笔记')).toBeEnabled();
	await page.getByLabel('单词笔记').fill('不能丢失的笔记');
	await page.getByRole('combobox', { name: '搜索英文单词' }).press('Enter');
	await expect(page.getByLabel('单词笔记')).toHaveValue('不能丢失的笔记');
	await page.evaluate(() => {
		(window as any).__saveFails = true;
	});
	await page.getByRole('button', { name: '保存笔记', exact: true }).click();
	await expect(page.getByRole('alert')).toContainText('笔记保存失败');
	await expect(page.getByLabel('单词笔记')).toHaveValue('不能丢失的笔记');
	await page.evaluate(() => {
		(window as any).__saveFails = false;
	});
	await page.getByRole('button', { name: '保存笔记', exact: true }).click();
	await expect(page.getByText('已保存在本地', { exact: true })).toBeVisible();
});

test('desktop contract: review prevents double submit and completes a session', async ({
	page
}) => {
	await desktopFixture(page);
	await page.goto('/review');
	const answer = page.getByLabel('你想到了哪个单词？');
	await expect(answer).toBeEnabled();
	await expect(page.getByRole('button', { name: /确认答案/ })).toBeDisabled();
	await answer.fill('meticulous');
	await answer.press('Enter');
	await page.keyboard.press('Enter');
	await expect(page.getByText('想起来了', { exact: true })).toBeVisible();
	const submissions = await page.evaluate(
		() => (window as any).__calls.filter((c: any) => c.command === 'submit_review').length
	);
	expect(submissions).toBe(1);
	await page.getByRole('button', { name: /下一个单词/ }).click();
	await expect(answer).toBeEnabled();
	await page.getByRole('button', { name: '暂时想不起来', exact: true }).click();
	await expect(page.getByText('再认识一次', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: /完成本次温习/ }).click();
	await expect(page.getByRole('heading', { name: '今天，又熟悉了一点。' })).toBeVisible();
});

test('desktop contract: ordered hints never reveal the next hint early', async ({ page }) => {
	await desktopFixture(page);
	await page.goto('/review');
	await page.getByRole('button', { name: /看看首字母/ }).click();
	await expect(page.getByText('m_________', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: /看看英文释义/ }).click();
	await expect(page.getByText('very careful about details', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: /看看中文释义/ }).click();
	await expect(page.getByText('一丝不苟的', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: /已展示全部提示/ })).toBeDisabled();
	await page.screenshot({ path: 'docs/screenshots/review-fixture.png', fullPage: true });
});

test('desktop contract: slow suggestion responses cannot replace newer input', async ({ page }) => {
	await desktopFixture(page);
	await page.goto('/');
	const input = page.getByRole('combobox', { name: '搜索英文单词' });
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
