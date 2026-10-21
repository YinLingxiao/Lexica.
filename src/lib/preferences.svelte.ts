import { browser } from '$app/environment';
type Theme = 'light' | 'dark' | 'system';
export type DesignEdition = 'obsidian' | 'cobalt' | 'vermilion';

export type ExtensionTabId =
	| 'examples'
	| 'mnemonic'
	| 'roots'
	| 'derivatives'
	| 'synonyms'
	| 'antonyms'
	| 'lookalikes'
	| 'forms';

export interface ExtensionTabConfig {
	id: Exclude<ExtensionTabId, 'examples'>;
	label: string;
	enabled: boolean;
}

export interface StudyPreferences {
	wordbook:
		'cet4' | 'cet6' | 'ky' | 'toefl' | 'ielts' | 'gre' | 'gk' | 'oxford' | 'custom' | 'encountered';
	cardMode: 'standard' | 'context';
	planStyle: 'standard' | 'intensive' | 'casual';
	dailyNewLimit: number;
	priority: 'mixed' | 'new_first' | 'review_first';
	autoPronounceWord: boolean;
	pronounceRepeat: 'never' | 'thrice';
	autoPronounceExample: boolean;
	autoPronounceContext: boolean;
	accent: 'US' | 'UK';
	speechRate: 0.8 | 1 | 1.2;
	exampleStyle: 'standard' | 'exam';
	exampleOrder: 'first' | 'random';
	autoSplitSyllables: boolean;
	fadeIn: boolean;
	hideChinese: boolean;
	inSessionQuiz: boolean;
	extensionTabs: ExtensionTabConfig[];
	autoReturnToExamples: boolean;
}

export const DAILY_NEW_LIMITS = [10, 20, 40, 60, 100] as const;

const DEFAULT_EXTENSION_TABS: ExtensionTabConfig[] = [
	{ id: 'mnemonic', label: '助记', enabled: true },
	{ id: 'roots', label: '词根词缀', enabled: true },
	{ id: 'derivatives', label: '派生', enabled: true },
	{ id: 'synonyms', label: '近义词', enabled: true },
	{ id: 'antonyms', label: '反义词', enabled: true },
	{ id: 'lookalikes', label: '形近词', enabled: true },
	{ id: 'forms', label: '词形变化', enabled: true }
];

export function defaultExtensionTabs(): ExtensionTabConfig[] {
	return DEFAULT_EXTENSION_TABS.map((tab) => ({ ...tab }));
}

export function defaultStudyPreferences(): StudyPreferences {
	return {
		wordbook: 'cet4',
		cardMode: 'standard',
		planStyle: 'standard',
		dailyNewLimit: 40,
		priority: 'mixed',
		autoPronounceWord: true,
		pronounceRepeat: 'never',
		autoPronounceExample: false,
		autoPronounceContext: false,
		accent: 'UK',
		speechRate: 1,
		exampleStyle: 'standard',
		exampleOrder: 'first',
		autoSplitSyllables: true,
		fadeIn: false,
		hideChinese: false,
		inSessionQuiz: false,
		extensionTabs: defaultExtensionTabs(),
		autoReturnToExamples: false
	};
}

function oneOf<T extends string | number>(value: unknown, allowed: readonly T[], fallback: T): T {
	return allowed.includes(value as T) ? (value as T) : fallback;
}

function boolOr(value: unknown, fallback: boolean): boolean {
	return typeof value === 'boolean' ? value : fallback;
}

function sanitizeExtensionTabs(value: unknown): ExtensionTabConfig[] {
	if (!Array.isArray(value)) return defaultExtensionTabs();
	const result: ExtensionTabConfig[] = [];
	// 按存档顺序还原；只接受已知 id，跳过重复项
	for (const item of value) {
		if (typeof item !== 'object' || item === null) continue;
		const { id, enabled } = item as { id?: unknown; enabled?: unknown };
		const tab = DEFAULT_EXTENSION_TABS.find((t) => t.id === id);
		if (!tab || result.some((t) => t.id === tab.id)) continue;
		result.push({ ...tab, enabled: boolOr(enabled, tab.enabled) });
	}
	// 新版本增加的页签按默认顺序补到末尾
	for (const tab of DEFAULT_EXTENSION_TABS) {
		if (!result.some((t) => t.id === tab.id)) result.push({ ...tab });
	}
	return result;
}

function sanitizeStudy(raw: unknown): StudyPreferences {
	const defaults = defaultStudyPreferences();
	const value = typeof raw === 'object' && raw !== null ? (raw as Record<string, unknown>) : {};
	return {
		wordbook: oneOf(
			value.wordbook,
			[
				'cet4',
				'cet6',
				'ky',
				'toefl',
				'ielts',
				'gre',
				'gk',
				'oxford',
				'custom',
				'encountered'
			] as const,
			defaults.wordbook
		),
		cardMode: oneOf(value.cardMode, ['standard', 'context'] as const, defaults.cardMode),
		planStyle: oneOf(
			value.planStyle,
			['standard', 'intensive', 'casual'] as const,
			defaults.planStyle
		),
		dailyNewLimit: oneOf(value.dailyNewLimit, DAILY_NEW_LIMITS, defaults.dailyNewLimit),
		priority: oneOf(
			value.priority,
			['mixed', 'new_first', 'review_first'] as const,
			defaults.priority
		),
		autoPronounceWord: boolOr(value.autoPronounceWord, defaults.autoPronounceWord),
		pronounceRepeat: oneOf(
			value.pronounceRepeat,
			['never', 'thrice'] as const,
			defaults.pronounceRepeat
		),
		autoPronounceExample: boolOr(value.autoPronounceExample, defaults.autoPronounceExample),
		autoPronounceContext: boolOr(value.autoPronounceContext, defaults.autoPronounceContext),
		accent: oneOf(value.accent, ['US', 'UK'] as const, defaults.accent),
		speechRate: oneOf(value.speechRate, [0.8, 1, 1.2] as const, defaults.speechRate),
		exampleStyle: oneOf(value.exampleStyle, ['standard', 'exam'] as const, defaults.exampleStyle),
		exampleOrder: oneOf(value.exampleOrder, ['first', 'random'] as const, defaults.exampleOrder),
		autoSplitSyllables: boolOr(value.autoSplitSyllables, defaults.autoSplitSyllables),
		fadeIn: boolOr(value.fadeIn, defaults.fadeIn),
		hideChinese: boolOr(value.hideChinese, defaults.hideChinese),
		inSessionQuiz: boolOr(value.inSessionQuiz, defaults.inSessionQuiz),
		extensionTabs: sanitizeExtensionTabs(value.extensionTabs),
		autoReturnToExamples: boolOr(value.autoReturnToExamples, defaults.autoReturnToExamples)
	};
}

export const preferences = $state({
	theme: 'system' as Theme,
	edition: 'obsidian' as DesignEdition,
	reading: 'guided',
	fontSize: 'normal',
	study: defaultStudyPreferences()
});

export function loadPreferences() {
	try {
		const value = JSON.parse(localStorage.getItem('lexica.preferences') ?? '{}');
		if (!value || typeof value !== 'object') return;
		if (['light', 'dark', 'system'].includes(value.theme)) preferences.theme = value.theme;
		if (['obsidian', 'cobalt', 'vermilion'].includes(value.edition))
			preferences.edition = value.edition;
		if (['guided', 'full'].includes(value.reading)) preferences.reading = value.reading;
		if (['normal', 'large'].includes(value.fontSize)) preferences.fontSize = value.fontSize;
		preferences.study = sanitizeStudy(value.study);
	} catch {
		/* Use defaults if storage is unavailable. */
	}
}

export function applyPreferences() {
	if (!browser) return;
	document.documentElement.dataset.theme = preferences.theme;
	document.documentElement.dataset.edition = preferences.edition;
	document.documentElement.dataset.fontSize = preferences.fontSize;
	try {
		localStorage.setItem('lexica.preferences', JSON.stringify(preferences));
	} catch {
		/* Applies for this session. */
	}
}
