// Development-only, read-only preview; desktop data is never simulated or changed.
import seed from '../../src-tauri/src/dictionary/seed/seed.json';
import type {
	Example,
	RecallQuality,
	RelatedWordItem,
	ReviewFeedback,
	ReviewItem,
	RootPart,
	StudyCardEntry,
	StudyGrade,
	StudyPriority,
	StudySessionGroup,
	WordEntry,
	WordFormEntry,
	WordbookInfo
} from './api';

const FORM_LABELS: Record<string, string> = {
	plural: 'Plural',
	past: 'Past',
	past_participle: 'Past participle',
	third_person: '3rd person singular',
	ing: 'Present participle',
	comparative: 'Comparative',
	superlative: 'Superlative',
	noun: 'Noun',
	verb: 'Verb',
	adjective: 'Adjective',
	adverb: 'Adverb'
};

const FORM_POS: Record<string, string> = {
	Noun: 'n.',
	Verb: 'v.',
	Adjective: 'adj.',
	Adverb: 'adv.'
};

function formsOf(raw: unknown): WordFormEntry[] {
	if (!raw || typeof raw !== 'object') return [];
	return Object.entries(raw as Record<string, unknown>)
		.filter(([, form]) => typeof form === 'string' && form.trim().length > 0)
		.map(([label, form]) => ({ label: FORM_LABELS[label] ?? label, form: String(form) }));
}

type RawSeedWord = (typeof seed.words)[number] & {
	tags?: string[];
	collins?: number;
	oxford?: boolean;
	mnemonic?: string;
	roots?: RootPart[];
	forms?: Record<string, string>;
};

const seedEntries: WordEntry[] = (seed.words as RawSeedWord[]).map((w, i) => ({
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
	word_family: w.word_family,
	tags: w.tags,
	collins: w.collins ?? null,
	oxford: w.oxford,
	mnemonic: w.mnemonic ?? null,
	roots: w.roots,
	forms: formsOf(w.forms)
}));

const MNEMONICS: Record<string, string> = {
	surgery:
		'surgery 来自希腊语 cheirourgia，cheir 表示手，ergon 意为工作，指用手操作的外科手术，在医疗领域至关重要。',
	ambiguous: 'ambi（两者）+ guous：两种解释都站得住，所以“模棱两可”。',
	elaborate: 'e（向外）+ labor（劳动）：把想法一层层做出来，就是“精心阐述”。',
	meticulous: '把 meticul 想成“每件事都抠细节”，-ous 表示充满——细节控。',
	resilient: 're（回）+ sil（跳）：被压弯后还能弹回来，就是“有韧性”。',
	pragmatic: 'prag（行动）+ matic：只看能不能落地，所以“务实”。',
	conspicuous: 'con + spic（看）：所有人一眼都看见，就是“显眼”。',
	inevitable: 'in（不）+ evit（避免）+ able：无法避免，就是“必然的”。'
};

const LOOKALIKES: Record<string, RelatedWordItem[]> = {
	surgery: [
		{ word: 'sugary', pos: 'adj.', gloss: '含糖的，过于甜蜜的' },
		{ word: 'surely', pos: 'adv.', gloss: '肯定地，不会吧，必定，当然' },
		{ word: 'surety', pos: 'n.', gloss: '保证金，担保人' }
	],
	religion: [
		{ word: 'region', pos: 'n.', gloss: '区域，地区，地方，部位' },
		{ word: 'relation', pos: 'n.', gloss: '关系，关联，亲属' },
		{ word: 'revision', pos: 'n.', gloss: '修改，修订，复习' }
	],
	meticulous: [
		{ word: 'ridiculous', pos: 'adj.', gloss: '荒谬的' },
		{ word: 'miraculous', pos: 'adj.', gloss: '奇迹般的' },
		{ word: 'tenacious', pos: 'adj.', gloss: '顽强的' }
	],
	elaborate: [
		{ word: 'collaborate', pos: 'v.', gloss: '合作' },
		{ word: 'labor', pos: 'n.', gloss: '劳动' },
		{ word: 'elaborately', pos: 'adv.', gloss: '精心地' }
	],
	ambiguous: [
		{ word: 'ambitious', pos: 'adj.', gloss: '有雄心的' },
		{ word: 'amphibious', pos: 'adj.', gloss: '两栖的' }
	],
	resilient: [
		{ word: 'resident', pos: 'n.', gloss: '居民' },
		{ word: 'resistant', pos: 'adj.', gloss: '抵抗的' }
	],
	pragmatic: [
		{ word: 'dogmatic', pos: 'adj.', gloss: '教条的' },
		{ word: 'dramatic', pos: 'adj.', gloss: '戏剧性的' }
	]
};

const SUFFIX_LOOKALIKES: Array<[string, string[]]> = [
	['ation', ['action', 'nation', 'station']],
	['tion', ['action', 'nation', 'mention']],
	['sion', ['vision', 'decision', 'mission']],
	['able', ['capable', 'stable', 'reliable']],
	['ible', ['visible', 'flexible', 'possible']],
	['ous', ['famous', 'nervous', 'obvious']],
	['ive', ['active', 'native', 'creative']],
	['ate', ['create', 'locate', 'estimate']],
	['ent', ['agent', 'silent', 'current']],
	['ant', ['giant', 'instant', 'constant']],
	['ity', ['quality', 'ability', 'activity']],
	['al', ['local', 'formal', 'normal']],
	['ic', ['basic', 'public', 'classic']],
	['ly', ['quickly', 'clearly', 'likely']],
	['y', ['happy', 'busy', 'easy']]
];

const PREFIX_ROOTS: Array<[string, string]> = [
	['under', '在下面；不足'],
	['super', '在上；超过'],
	['inter', '在…之间'],
	['trans', '穿过；转移'],
	['dis', '分开；否定'],
	['pre', '在前；预先'],
	['pro', '向前；支持'],
	['con', '共同；一起'],
	['com', '共同；一起'],
	['sub', '在下面；次级'],
	['sur', '在上面；超过'],
	['per', '贯穿；彻底'],
	['out', '向外；超过'],
	['in', '不；进入'],
	['im', '不（in- 的变体）'],
	['un', '不；否定'],
	['re', '再；向后'],
	['de', '向下；去除'],
	['ex', '向外；出'],
	['ad', '朝向；加强']
];

const SUFFIX_ROOTS: Array<[string, string]> = [
	['ation', '名词后缀，表示行为或结果'],
	['ition', '名词后缀，表示行为或结果'],
	['tion', '名词后缀，表示行为或结果'],
	['sion', '名词后缀，表示行为或结果'],
	['ment', '名词后缀，表示行为、状态或结果'],
	['ness', '名词后缀，表示性质或状态'],
	['ance', '名词后缀，表示状态或行为'],
	['ence', '名词后缀，表示状态或行为'],
	['ity', '名词后缀，表示性质'],
	['ous', '形容词后缀，充满…的'],
	['ive', '形容词后缀，有…性质的'],
	['able', '形容词后缀，能够…的'],
	['ible', '形容词后缀，能够…的'],
	['ical', '形容词后缀'],
	['ic', '形容词后缀'],
	['al', '形容词后缀'],
	['ent', '形容词或名词后缀'],
	['ant', '形容词或名词后缀'],
	['ful', '形容词后缀，充满…的'],
	['less', '形容词后缀，缺少…的'],
	['ize', '动词后缀，使…'],
	['ise', '动词后缀，使…'],
	['ify', '动词后缀，使…'],
	['ate', '动词或形容词后缀'],
	['ly', '副词后缀'],
	['er', '名词后缀，表示人或物'],
	['or', '名词后缀，表示人或物'],
	['ing', '现在分词或动名词后缀'],
	['ed', '过去式或过去分词后缀']
];

function firstSense(entry: WordEntry) {
	return entry.senses[0] ?? null;
}

function firstChinese(entry: WordEntry): string {
	return firstSense(entry)?.chinese_definition ?? firstSense(entry)?.english_definition ?? '';
}

function firstPos(entry: WordEntry): string | null {
	return firstSense(entry)?.pos ?? null;
}

function firstExample(entry: WordEntry): Example | null {
	for (const sense of entry.senses) {
		if (sense.examples.length) return sense.examples[0];
	}
	return null;
}

function tagsOf(entry: WordEntry): string[] {
	const sense = firstSense(entry);
	const tags = [sense?.level, sense?.pos].filter((tag): tag is string => !!tag);
	return tags.length ? tags : ['核心词'];
}

function collinsOf(entry: WordEntry): number | null {
	const rank = entry.frequency_rank;
	if (rank === null) return null;
	if (rank <= 4000) return 5;
	if (rank <= 6000) return 4;
	if (rank <= 8000) return 3;
	if (rank <= 12000) return 2;
	return 1;
}

function mnemonicOf(entry: WordEntry): string {
	return (
		MNEMONICS[entry.word] ??
		`联想记忆：把 ${entry.word} 和「${firstChinese(entry) || '它的含义'}」放进同一个画面里，画面越具体越好记。`
	);
}

function rootsOf(entry: WordEntry): RootPart[] {
	const word = entry.word.toLowerCase();
	const roots: RootPart[] = [];
	const prefix = PREFIX_ROOTS.find(
		([part]) => word.startsWith(part) && word.length > part.length + 2
	);
	if (prefix) roots.push({ part: `${prefix[0]}-`, meaning: prefix[1] });
	const suffix = SUFFIX_ROOTS.find(
		([part]) => word.endsWith(part) && word.length > part.length + 2
	);
	if (suffix) roots.push({ part: `-${suffix[0]}`, meaning: suffix[1] });
	if (!roots.length) roots.push({ part: word, meaning: firstChinese(entry) || '词根' });
	return roots;
}

function derivativesOf(entry: WordEntry): RelatedWordItem[] {
	const out: RelatedWordItem[] = [];
	const push = (word: string, pos: string | null, gloss: string) => {
		const clean = word.trim();
		if (!clean || clean === entry.word) return;
		if (!out.some((item) => item.word === clean)) out.push({ word: clean, pos, gloss });
	};
	for (const family of entry.word_family) {
		const inline = /^(\S+)\s+((?:n|v|vt|vi|adj|adv|prep|conj|pron)\.)\s*(.*)$/i.exec(family.trim());
		if (inline) push(inline[1], inline[2], inline[3].trim() || '派生词');
		else push(family, null, '派生词');
	}
	for (const form of entry.forms ?? []) {
		const pos = FORM_POS[form.label];
		if (pos) push(form.form, pos, '派生词');
	}
	return out;
}

function detailList(words: string[], entry: WordEntry, fallbackGloss: string): RelatedWordItem[] {
	const gloss = firstChinese(entry) || fallbackGloss;
	return words.map((word) => ({ word, pos: firstPos(entry), gloss }));
}

function lookalikesOf(word: string): RelatedWordItem[] {
	const manual = LOOKALIKES[word];
	if (manual) return manual.map((item) => ({ ...item }));
	const bucket = SUFFIX_LOOKALIKES.find(([suffix]) => word.endsWith(suffix));
	return (bucket?.[1] ?? []).map((item) => ({ word: item, pos: null, gloss: '形近词' }));
}

function enrich(entry: WordEntry): WordEntry {
	return {
		...entry,
		tags: entry.tags ?? tagsOf(entry),
		collins: entry.collins ?? collinsOf(entry),
		oxford: entry.oxford ?? (entry.frequency_rank !== null && entry.frequency_rank <= 7000),
		mnemonic: entry.mnemonic ?? mnemonicOf(entry),
		roots: entry.roots?.length ? entry.roots : rootsOf(entry),
		derivatives: entry.derivatives ?? derivativesOf(entry),
		synonyms_detailed: entry.synonyms_detailed ?? detailList(entry.synonyms, entry, '近义词'),
		antonyms_detailed: entry.antonyms_detailed ?? detailList(entry.antonyms, entry, '反义词'),
		lookalikes: entry.lookalikes ?? lookalikesOf(entry.word),
		forms: entry.forms ?? [],
		exam_example: entry.exam_example ?? firstExample(entry)
	};
}

const entries: WordEntry[] = seedEntries.map(enrich);

function cloze(text: string, word: string): string {
	const escaped = word.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
	return text.replace(new RegExp(`\\b${escaped}\\b`, 'gi'), '____');
}

function studyItem(entry: WordEntry): ReviewItem {
	const example = entry.exam_example ?? firstExample(entry);
	const sense = firstSense(entry);
	return {
		word_id: entry.id,
		prompt: example ? cloze(example.text, entry.word) : (sense?.english_definition ?? entry.word),
		kind: example ? 'cloze' : 'definition',
		source: 'dictionary',
		sense_id: sense?.id ?? null,
		example_id: null
	};
}

function studyCards(limit: number, wordbook: string, priority: StudyPriority): StudyCardEntry[] {
	const surgery = entries.find((e) => e.word === 'surgery');
	const rest = entries.filter((e) => e.word !== 'surgery');
	const pool = (surgery ? [surgery, ...rest] : entries).slice(0, limit);
	const newCount = Math.max(1, Math.ceil(pool.length * 0.6));
	const cards = pool.map((entry, index) => ({
		entry,
		item: studyItem(entry),
		is_new: index < newCount,
		bookmarked: index % 6 === 0,
		in_custom_wordbook: wordbook === 'custom' || index % 4 === 0
	}));
	if (priority === 'new_first') {
		cards.sort((a, b) => Number(b.is_new) - Number(a.is_new));
	} else if (priority === 'review_first') {
		cards.sort((a, b) => Number(a.is_new) - Number(b.is_new));
	}
	return cards;
}

const WORD_BOOKS: Array<Omit<WordbookInfo, 'learned_words'> & { learned: number }> = [
	{ id: 'cet4', name: 'CET-4 四级核心词', total_words: 3849, learned: 826 },
	{ id: 'cet6', name: 'CET-6 六级核心词', total_words: 2540, learned: 418 },
	{ id: 'ky', name: '考研大纲词', total_words: 5500, learned: 1204 },
	{ id: 'toefl', name: 'TOEFL 托福核心词', total_words: 4464, learned: 372 },
	{ id: 'ielts', name: 'IELTS 雅思核心词', total_words: 4175, learned: 286 },
	{ id: 'gre', name: 'GRE 核心词', total_words: 3200, learned: 164 },
	{ id: 'gk', name: '高考核心词', total_words: 3500, learned: 1580 },
	{ id: 'oxford', name: '牛津 3000 核心词', total_words: 3000, learned: 912 },
	{ id: 'custom', name: '我的生词本', total_words: 128, learned: 64 },
	{ id: 'encountered', name: '查词本', total_words: 96, learned: 96 }
];

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
				word_count: 50,
				schema_version: 5,
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
		case 'study_group': {
			const limit = Math.max(1, Math.min(entries.length, Number(args.dailyNewLimit) || 20));
			const priority = String(args.priority ?? 'mixed') as StudyPriority;
			const wordbook = String(args.wordbook ?? 'cet4');
			const cards = studyCards(limit, wordbook, priority);
			const newCount = cards.filter((card) => card.is_new).length;
			const group: StudySessionGroup = {
				cards,
				due_count: cards.length - newCount,
				new_count: newCount,
				today_learned_new: Math.min(12, newCount)
			};
			return group;
		}
		case 'wordbooks_list':
			return WORD_BOOKS.map((book) => ({
				id: book.id,
				name: book.name,
				total_words: book.total_words,
				learned_words: book.learned
			}));
		case 'study_grade': {
			const grade = String(args.grade ?? 'know') as StudyGrade;
			const quality: RecallQuality =
				grade === 'forgot' ? 'forgotten' : grade === 'hazy' ? 'hard' : 'good';
			const feedback: ReviewFeedback = {
				correct: grade !== 'forgot',
				quality,
				answer: '',
				hints_used: 0,
				memory: {
					word_id: Number(args.wordId ?? 0),
					strength_days: grade === 'know' ? 3 : grade === 'hazy' ? 1 : 0,
					difficulty: grade === 'forgot' ? 7 : 5,
					review_count: 1,
					lapse_count: grade === 'forgot' ? 1 : 0,
					visit_count: 1,
					weak_streak: grade === 'forgot' ? 1 : 0,
					high_priority: grade === 'forgot',
					next_review_at: null
				}
			};
			return feedback;
		}
		case 'custom_wordbook_toggle':
			return undefined;
		case 'ai_enrich_words':
			return Array.isArray(args.wordIds) ? args.wordIds.length : 0;
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
