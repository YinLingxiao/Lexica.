// TypeScript mirror of the Rust DTOs plus a thin invoke wrapper.
// All business logic lives in Rust; the frontend is a pure view.
import { invoke as tauriInvoke, isTauri } from '@tauri-apps/api/core';

export const isPreview = () => !isTauri();
export function errorMessage(error: unknown): string {
	if (error && typeof error === 'object' && 'message' in error) return String(error.message);
	return typeof error === 'string' ? error : 'Something went wrong. Try again.';
}
async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
	try {
		if (!isTauri()) {
			if (import.meta.env.DEV)
				return (await import('./preview')).previewInvoke(command, args) as Promise<T>;
			throw new Error('Open the desktop app to connect the local dictionary.');
		}
		return await tauriInvoke<T>(command, args);
	} catch (error) {
		throw new Error(errorMessage(error));
	}
}

export interface WordNote {
	bookmarked: boolean;
	note: string;
	ignored: boolean;
}
export interface LibraryWord {
	id: number;
	word: string;
	display: string;
	phonetic: string | null;
	definition: string;
	status: string;
	bookmarked: boolean;
	note: string;
	visit_count: number;
	next_review_at: string | null;
}
export interface LibraryPage {
	words: LibraryWord[];
	total: number;
}
export interface ActivityDay {
	date: string;
	lookups: number;
	reviews: number;
}
export const wordNote = (wordId: number) => invoke<WordNote>('word_note', { wordId });
export const saveWordNote = (wordId: number, bookmarked: boolean, note: string) =>
	invoke<void>('save_word_note', { wordId, bookmarked, note });
export const libraryWords = (query = '', filter = 'all', offset = 0) =>
	invoke<LibraryPage>('library_words', { query, filter, offset });
export const activityDays = () =>
	invoke<ActivityDay[]>('activity_days', { offsetMinutes: -new Date().getTimezoneOffset() });
export const statusLabel: Record<string, string> = {
	new: 'New',
	learning: 'Learning',
	familiar: 'Familiar',
	stable: 'Stable',
	ignored: 'Paused'
};

export type ComprehensionLevel = 'context' | 'english' | 'chinese' | 'unknown';

export interface AppInfo {
	schema_version: number;
	word_count: number;
	fts_ok: boolean;
	dictionary_ready: boolean;
}

export interface Suggestion {
	word: string;
	display: string;
	phonetic: string | null;
	frequency_rank: number | null;
}

export interface Example {
	text: string;
	translation: string | null;
}

export interface Collocation {
	text: string;
	gloss: string | null;
}

export interface Sense {
	id: number;
	pos: string | null;
	english_definition: string;
	chinese_definition: string | null;
	level: string | null;
	examples: Example[];
}

export interface WordEntry {
	id: number;
	word: string;
	display: string;
	phonetic: string | null;
	senses: Sense[];
	collocations: Collocation[];
	synonyms: string[];
	antonyms: string[];
	word_family: string[];
	frequency_rank: number | null;
}

export type LookupOutcome =
	{ type: 'hit'; entry: WordEntry } | { type: 'miss'; suggestions: Suggestion[] };

export type MemoryStatus = 'learning' | 'familiar' | 'stable';

export interface MemorySummary {
	status: MemoryStatus | null;
	visit_count: number;
	high_priority: boolean;
}

export interface LookupWordResult {
	outcome: LookupOutcome;
	encounter_id: number | null;
	memory: MemorySummary | null;
	comprehension_level?: ComprehensionLevel | null;
}

export interface StatsSummary {
	encountered: number;
	learning: number;
	familiar: number;
	stable: number;
	ignored: number;
	due_now: number;
	high_priority: number;
}

export interface RecentWord {
	word: string;
	display: string;
	phonetic: string | null;
	visited_at: string;
	comprehension_level: ComprehensionLevel;
}

export async function appInfo(): Promise<AppInfo> {
	return invoke<AppInfo>('app_info');
}

/** Imports the full ECDICT database (stardict.db); returns newly added entries. Takes minutes. */
export async function importEcdict(sourcePath: string): Promise<number> {
	return invoke<number>('import_ecdict', { sourcePath });
}

export type SetupPhase =
	'checking' | 'downloading' | 'extracting' | 'importing' | 'ready' | 'error';

export interface SetupProgress {
	phase: SetupPhase;
	progress: number | null;
	message: string;
}

export interface EnsureDictionaryResult {
	ready: boolean;
	word_count: number;
	imported: number;
	skipped: boolean;
}

/** Download ECDICT when missing, then import. Emits `dictionary-setup` progress events. */
export async function ensureDictionary(): Promise<EnsureDictionaryResult> {
	return invoke<EnsureDictionaryResult>('ensure_dictionary');
}

export async function searchSuggest(query: string, limit = 8): Promise<Suggestion[]> {
	return invoke<Suggestion[]>('search_suggest', { query, limit });
}

export async function lookupWord(word: string): Promise<LookupWordResult> {
	return invoke<LookupWordResult>('lookup_word', { word });
}

export async function setComprehension(
	encounterId: number,
	level: ComprehensionLevel
): Promise<void> {
	return invoke('set_comprehension', { encounterId, level });
}

export async function recentWords(limit = 6): Promise<RecentWord[]> {
	return invoke<RecentWord[]>('recent_words', { limit });
}

export async function statsSummary(): Promise<StatsSummary> {
	return invoke<StatsSummary>('stats_summary');
}

export async function setWordIgnored(wordId: number, ignored: boolean): Promise<void> {
	return invoke('set_word_ignored', { wordId, ignored });
}

// ── Review ────────────────────────────────────────────────

/** Prompt type (explicit, not inferred from example_id). */
export type PromptKind = 'cloze' | 'definition' | 'definition_zh';
/** Prompt source: dictionary original or AI supplement. */
export type PromptSource = 'dictionary' | 'ai';

export interface ReviewItem {
	word_id: number;
	/** Cloze sentence, English definition, or Chinese definition fallback. */
	prompt: string;
	kind: PromptKind;
	source: PromptSource;
	sense_id: number | null;
	/** Always null for AI prompts — dictionary example ids are never faked. */
	example_id: number | null;
}

/** Read-only review group entry: full entry for browsing + the local prompt. */
export interface ReviewGroupEntry {
	item: ReviewItem;
	entry: WordEntry;
}

export type Hint =
	| { kind: 'mask'; text: string }
	| { kind: 'english'; text: string }
	| { kind: 'chinese'; text: string };

export interface MemoryStateLite {
	word_id: number;
	strength_days: number;
	difficulty: number;
	review_count: number;
	lapse_count: number;
	visit_count: number;
	weak_streak: number;
	high_priority: boolean;
	next_review_at: string | null;
}

export type RecallQuality = 'excellent' | 'good' | 'hard' | 'poor' | 'forgotten';

export interface ReviewFeedback {
	correct: boolean;
	quality: RecallQuality;
	answer: string;
	hints_used: number;
	memory: MemoryStateLite | null;
}

export interface FadingWord {
	word: string;
	display: string;
	high_priority: boolean;
	next_review_at: string | null;
}

export async function reviewQueue(): Promise<ReviewItem[]> {
	return invoke<ReviewItem[]>('review_queue');
}

/** Read-only group of due words (≤10): full entries + local prompts. No side effects. */
export async function reviewGroup(): Promise<ReviewGroupEntry[]> {
	return invoke<ReviewGroupEntry[]>('review_group');
}

export async function startReviewItem(item: ReviewItem): Promise<number> {
	return invoke<number>('start_review_item', {
		wordId: item.word_id,
		senseId: item.sense_id,
		exampleId: item.example_id
	});
}

/** In-group retry grading: read-only, reuses the Rust grader, never double-counts. */
export async function practiceCheck(
	wordId: number,
	answer: string | null
): Promise<{ correct: boolean }> {
	return invoke('practice_check', { wordId, answer });
}

/** In-group retry hints: read-only, same construction as the formal flow. */
export async function practiceHint(
	wordId: number,
	senseId: number | null,
	hintNo: number
): Promise<Hint> {
	return invoke('practice_hint', { wordId, senseId, hintNo });
}

// ── AI example sentences (optional, off by default) ───────

export interface AiConfigView {
	enabled: boolean;
	base_url: string;
	model: string;
	/** Whether a key is stored — the key itself is never returned. */
	has_key: boolean;
}

export interface AiExample {
	word_id: number;
	sense_id: number | null;
	/** Validated, target-word-blanked sentence, ready to use as a prompt. */
	sentence: string;
}

export interface AiGenItem {
	word_id: number;
	sense_id: number | null;
}

export async function aiConfigGet(): Promise<AiConfigView> {
	return invoke<AiConfigView>('ai_config_get');
}

/**
 * Saves AI settings. `key`: null keeps the stored key, '' deletes it,
 * a non-empty string replaces it. The key lives in the OS credential store.
 */
export async function aiConfigSave(
	enabled: boolean,
	baseUrl: string,
	model: string,
	key: string | null
): Promise<AiConfigView> {
	return invoke<AiConfigView>('ai_config_save', { enabled, baseUrl, model, key });
}

export async function aiTestConnection(): Promise<void> {
	return invoke('ai_test_connection');
}

/** Generate and cache cloze-ready AI examples. Callers choose when generation is needed. */
export async function aiGenerateExamples(items: AiGenItem[]): Promise<AiExample[]> {
	return invoke<AiExample[]>('ai_generate_examples', { items });
}

export async function requestHint(reviewId: number, hintNo: number): Promise<Hint> {
	return invoke<Hint>('request_hint', { reviewId, hintNo });
}

export async function submitReview(
	reviewId: number,
	answer: string | null
): Promise<ReviewFeedback> {
	return invoke<ReviewFeedback>('submit_review', { reviewId, answer });
}

export async function fadingWords(limit = 20): Promise<FadingWord[]> {
	return invoke<FadingWord[]>('fading_words', { limit });
}

export function dueLabel(iso: string | null): string {
	if (!iso) return '';
	const ms = new Date(iso).getTime() - Date.now();
	if (!Number.isFinite(ms)) return '';
	if (ms <= 0) return 'Due';
	if (ms < 3_600_000) return 'in under 1h';
	if (ms < 86_400_000) return `in ${Math.ceil(ms / 3_600_000)}h`;
	const d = Math.ceil(ms / 86_400_000);
	return `in ${d}d`;
}

export function relTime(iso: string): string {
	const ms = Date.now() - new Date(iso).getTime();
	if (!Number.isFinite(ms)) return '';
	const min = Math.floor(ms / 60_000);
	if (min < 1) return 'just now';
	if (min < 60) return `${min}m ago`;
	const h = Math.floor(min / 60);
	if (h < 24) return `${h}h ago`;
	return `${Math.floor(h / 24)}d ago`;
}
