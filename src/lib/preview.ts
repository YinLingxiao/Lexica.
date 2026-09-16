// Development-only, read-only preview; desktop data is never simulated or changed.
import seed from '../../src-tauri/src/dictionary/seed/seed.json';
import type { WordEntry } from './api';
const entries: WordEntry[] = seed.words.map((w, i) => ({
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
export async function previewInvoke(
	command: string,
	args: Record<string, unknown> = {}
): Promise<unknown> {
	const query = String(args.query ?? args.word ?? '')
		.trim()
		.toLowerCase();
	const suggestions = entries
		.filter((w) => w.word.startsWith(query))
		.slice(0, Number(args.limit ?? 8));
	switch (command) {
		case 'app_info':
			return {
				word_count: entries.length,
				schema_version: 3,
				fts_ok: true,
				dictionary_ready: true
			};
		case 'search_suggest':
			return suggestions;
		case 'lookup_word': {
			const entry = entries.find((w) => w.word === query);
			return {
				outcome: entry ? { type: 'hit', entry } : { type: 'miss', suggestions },
				encounter_id: null,
				memory: null
			};
		}
		case 'stats_summary':
			return {
				encountered: 0,
				learning: 0,
				familiar: 0,
				stable: 0,
				ignored: 0,
				due_now: 0,
				high_priority: 0
			};
		case 'word_note':
			return { bookmarked: false, note: '', ignored: false };
		case 'library_words':
			return { words: [], total: 0 };
		case 'recent_words':
		case 'review_queue':
		case 'fading_words':
			return [];
		case 'activity_days':
			return Array.from({ length: 14 }, (_, i) => {
				const d = new Date();
				d.setDate(d.getDate() - 13 + i);
				return {
					date: `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`,
					lookups: 0,
					reviews: 0
				};
			});
		case 'ensure_dictionary':
			return { ready: true, word_count: entries.length, imported: 0, skipped: true };
		case 'review_group':
			return [];
		case 'ai_config_get':
			return {
				enabled: false,
				base_url: 'https://api.deepseek.com/v1',
				model: 'deepseek-chat',
				has_key: false
			};
		default:
			throw new Error('Browser preview is read-only. Use the desktop app for this action.');
	}
}
