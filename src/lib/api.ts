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

export interface ReviewItem {
	word_id: number;
	/** Cloze sentence, or the English definition when no example exists. */
	prompt: string;
	sense_id: number | null;
	example_id: number | null;
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

export async function startReviewItem(item: ReviewItem): Promise<number> {
	return invoke<number>('start_review_item', {
		wordId: item.word_id,
		senseId: item.sense_id,
		exampleId: item.example_id
	});
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
