// English syllable splitting — a small exact dictionary for high-frequency
// words plus a general vowel-cluster / consonant-bridging fallback.

const DOT = '\u00B7';

const VOWELS = 'aeiou';

/**
 * Curated syllabifications for words where the fallback heuristic would
 * produce a questionable break. Keys are lowercase.
 */
const EXACT: Record<string, string[]> = {
	acquire: ['ac', 'quire'],
	aesthetic: ['aes', 'thet', 'ic'],
	ambiguous: ['am', 'big', 'u', 'ous'],
	answer: ['an', 'swer'],
	application: ['ap', 'pli', 'ca', 'tion'],
	apple: ['ap', 'ple'],
	beautiful: ['beau', 'ti', 'ful'],
	because: ['be', 'cause'],
	between: ['be', 'tween'],
	book: ['book'],
	business: ['busi', 'ness'],
	children: ['chil', 'dren'],
	celebration: ['cel', 'e', 'bra', 'tion'],
	coherent: ['co', 'her', 'ent'],
	color: ['col', 'or'],
	comfortable: ['com', 'fort', 'a', 'ble'],
	communication: ['com', 'mu', 'ni', 'ca', 'tion'],
	compelling: ['com', 'pel', 'ling'],
	computer: ['com', 'put', 'er'],
	conspicuous: ['con', 'spic', 'u', 'ous'],
	country: ['coun', 'try'],
	courageous: ['cou', 'ra', 'geous'],
	create: ['cre', 'ate'],
	culture: ['cul', 'ture'],
	deteriorate: ['de', 'te', 'ri', 'o', 'rate'],
	development: ['de', 'vel', 'op', 'ment'],
	dictionary: ['dic', 'tion', 'ar', 'y'],
	difficult: ['dif', 'fi', 'cult'],
	different: ['dif', 'fer', 'ent'],
	diminish: ['di', 'min', 'ish'],
	dwindle: ['dwin', 'dle'],
	economy: ['e', 'con', 'o', 'my'],
	education: ['ed', 'u', 'ca', 'tion'],
	elaborate: ['e', 'lab', 'o', 'rate'],
	endeavor: ['en', 'deav', 'or'],
	english: ['eng', 'lish'],
	enough: ['e', 'nough'],
	environment: ['en', 'vi', 'ron', 'ment'],
	evening: ['eve', 'ning'],
	example: ['ex', 'am', 'ple'],
	exhaustive: ['ex', 'haus', 'tive'],
	experience: ['ex', 'pe', 'ri', 'ence'],
	extraordinary: ['ex', 'traor', 'di', 'nar', 'y'],
	family: ['fam', 'i', 'ly'],
	favorite: ['fa', 'vor', 'ite'],
	feasible: ['fea', 'si', 'ble'],
	friend: ['friend'],
	government: ['gov', 'ern', 'ment'],
	happiness: ['hap', 'pi', 'ness'],
	hinder: ['hin', 'der'],
	history: ['his', 'to', 'ry'],
	house: ['house'],
	imperative: ['im', 'per', 'a', 'tive'],
	important: ['im', 'por', 'tant'],
	inevitable: ['in', 'ev', 'i', 'ta', 'ble'],
	inherent: ['in', 'her', 'ent'],
	information: ['in', 'for', 'ma', 'tion'],
	interesting: ['in', 'ter', 'est', 'ing'],
	international: ['in', 'ter', 'na', 'tion', 'al'],
	intricate: ['in', 'tri', 'cate'],
	knowledge: ['knowl', 'edge'],
	language: ['lan', 'guage'],
	laugh: ['laugh'],
	learning: ['learn', 'ing'],
	letter: ['let', 'ter'],
	life: ['life'],
	linger: ['lin', 'ger'],
	little: ['lit', 'tle'],
	memory: ['mem', 'o', 'ry'],
	magnificent: ['mag', 'nif', 'i', 'cent'],
	method: ['meth', 'od'],
	meticulous: ['me', 'tic', 'u', 'lous'],
	mitigate: ['mit', 'i', 'gate'],
	morning: ['morn', 'ing'],
	music: ['mu', 'sic'],
	nature: ['na', 'ture'],
	necessary: ['nec', 'es', 'sar', 'y'],
	novice: ['nov', 'ice'],
	number: ['num', 'ber'],
	obsolete: ['ob', 'so', 'lete'],
	opportunity: ['op', 'por', 'tu', 'ni', 'ty'],
	organization: ['or', 'gan', 'i', 'za', 'tion'],
	orange: ['or', 'ange'],
	outweigh: ['out', 'weigh'],
	people: ['peo', 'ple'],
	person: ['per', 'son'],
	pervasive: ['per', 'va', 'sive'],
	plausible: ['plau', 'si', 'ble'],
	possible: ['pos', 'si', 'ble'],
	practice: ['prac', 'tice'],
	pragmatic: ['prag', 'mat', 'ic'],
	prevalent: ['prev', 'a', 'lent'],
	problem: ['prob', 'lem'],
	process: ['proc', 'ess'],
	profound: ['pro', 'found'],
	program: ['pro', 'gram'],
	pronunciation: ['pro', 'nun', 'ci', 'a', 'tion'],
	question: ['ques', 'tion'],
	refute: ['re', 'fute'],
	reluctant: ['re', 'luc', 'tant'],
	resilient: ['re', 'sil', 'ient'],
	result: ['re', 'sult'],
	review: ['re', 'view'],
	reviewing: ['re', 'view', 'ing'],
	rigorous: ['rig', 'or', 'ous'],
	school: ['school'],
	science: ['sci', 'ence'],
	scrutinize: ['scru', 'ti', 'nize'],
	sentence: ['sen', 'tence'],
	simple: ['sim', 'ple'],
	skeptical: ['skep', 'ti', 'cal'],
	sophisticated: ['so', 'phis', 'ti', 'cat', 'ed'],
	spontaneous: ['spon', 'ta', 'ne', 'ous'],
	strenuous: ['stren', 'u', 'ous'],
	student: ['stu', 'dent'],
	study: ['stud', 'y'],
	subtle: ['sub', 'tle'],
	surgery: ['sur', 'ge', 'ry'],
	surge: ['surge'],
	syllabification: ['syl', 'lab', 'i', 'fi', 'ca', 'tion'],
	syllable: ['syl', 'la', 'ble'],
	system: ['sys', 'tem'],
	table: ['ta', 'ble'],
	tangible: ['tan', 'gi', 'ble'],
	teacher: ['teach', 'er'],
	technology: ['tech', 'nol', 'o', 'gy'],
	tedious: ['te', 'di', 'ous'],
	tentative: ['ten', 'ta', 'tive'],
	thought: ['thought'],
	thoughtful: ['thought', 'ful'],
	though: ['though'],
	through: ['through'],
	thrive: ['thrive'],
	time: ['time'],
	trivial: ['triv', 'i', 'al'],
	understand: ['un', 'der', 'stand'],
	undermine: ['un', 'der', 'mine'],
	university: ['u', 'ni', 'ver', 'si', 'ty'],
	utter: ['ut', 'ter'],
	verify: ['ver', 'i', 'fy'],
	versatile: ['ver', 'sa', 'tile'],
	vigilant: ['vig', 'i', 'lant'],
	vocabulary: ['vo', 'cab', 'u', 'lar', 'y'],
	water: ['wa', 'ter'],
	wary: ['war', 'y'],
	wonderful: ['won', 'der', 'ful'],
	work: ['work'],
	world: ['world'],
	year: ['year'],
	yield: ['yield']
};

/** Three-consonant clusters that can start a syllable. */
const ONSETS3 = new Set(['str', 'spr', 'scr', 'spl', 'shr', 'thr', 'chr', 'phr', 'squ', 'sph']);

function isVowel(word: string, index: number): boolean {
	const ch = word[index];
	if (VOWELS.includes(ch)) return true;
	if (ch !== 'y') return false;
	if (index === 0) return false;
	const prev = word[index - 1];
	return !'aeiouy'.includes(prev);
}

interface Nucleus {
	start: number;
	end: number;
}

function nucleiOf(word: string): Nucleus[] {
	const nuclei: Nucleus[] = [];
	for (let i = 0; i < word.length; i++) {
		if (!isVowel(word, i)) continue;
		let end = i + 1;
		while (end < word.length && isVowel(word, end)) end++;
		nuclei.push({ start: i, end });
		i = end - 1;
	}
	if (nuclei.length > 1) {
		const last = nuclei[nuclei.length - 1];
		const isFinalE = word.endsWith('e') && last.start === word.length - 1;
		if (isFinalE && word.length >= 4 && !/(?:ee|ye|oe)$/.test(word)) nuclei.pop();
	}
	return nuclei;
}

function isConsonant(ch: string | undefined): boolean {
	return !!ch && /[a-z]/i.test(ch) && !VOWELS.includes(ch.toLowerCase());
}

function splitAlpha(input: string): string[] {
	const word = input.toLowerCase();
	if (word.length <= 3) return [input];
	const exact = EXACT[word];
	if (exact) return matchCase(input, exact);
	if (![...word].some((ch) => VOWELS.includes(ch) || ch === 'y')) return [input];

	const nuclei = nucleiOf(word);
	if (nuclei.length === 0) return [input];

	const cuts = new Set<number>();

	for (let i = 0; i < nuclei.length - 1; i++) {
		const prevEnd = nuclei[i].end;
		const nextStart = nuclei[i + 1].start;
		const clusterLength = nextStart - prevEnd;
		let cut: number;
		if (clusterLength <= 1) {
			// A single consonant normally starts the next syllable (V/CV).
			cut = prevEnd;
		} else if (clusterLength === 2) {
			cut = prevEnd + 1;
		} else {
			const tail = word.slice(nextStart - 3, nextStart);
			cut = ONSETS3.has(tail) ? nextStart - 3 : prevEnd + 1;
		}
		if (cut > 0 && cut < word.length) cuts.add(cut);
	}

	// Syllabic "-le": ta·ble, sim·ple, lit·tle.
	if (word.length >= 4 && word.endsWith('le') && isConsonant(word[word.length - 3])) {
		const cut = word.length - 3;
		if (cut > 0 && cut < word.length) cuts.add(cut);
	}

	const ordered = [...cuts].sort((a, b) => a - b);
	const parts: string[] = [];
	let cursor = 0;
	for (const cut of ordered) {
		if (cut <= cursor) continue;
		parts.push(word.slice(cursor, cut));
		cursor = cut;
	}
	parts.push(word.slice(cursor));
	return matchCase(input, parts.filter(Boolean));
}

function matchCase(input: string, parts: string[]): string[] {
	if (input.length > 1 && input === input.toUpperCase() && /[A-Z]/.test(input)) {
		return parts.map((part) => part.toUpperCase());
	}
	if (/^[A-Z]/.test(input)) {
		return parts.map((part, index) =>
			index === 0 ? part.charAt(0).toUpperCase() + part.slice(1) : part
		);
	}
	return parts;
}

/** Splits an English word into syllables (without separators). */
export function splitSyllables(word: string): string[] {
	const trimmed = word.trim();
	if (!trimmed) return [];
	const runs = trimmed.match(/[A-Za-z]+(?:['’][A-Za-z]+)?/g);
	if (!runs) return [trimmed];
	const result: string[] = [];
	for (const run of runs) {
		const apostrophe = run.search(/['’]/);
		if (apostrophe > 0) {
			result.push(...splitAlpha(run.slice(0, apostrophe)));
			result.push(...splitAlpha(run.slice(apostrophe + 1)));
		} else {
			result.push(...splitAlpha(run));
		}
	}
	return result.length ? result : [trimmed];
}

/** Returns the headword with middle-dot separators when autoSplit is on. */
export function formatHeadword(word: string, autoSplit: boolean): string {
	if (!autoSplit) return word;
	const trimmed = word.trim();
	if (!trimmed) return word;
	if (/[\s-]/.test(trimmed)) {
		return trimmed
			.split(/(\s+|-)/)
			.map((part) => (/^[A-Za-z]+$/.test(part) ? splitSyllables(part).join(DOT) : part))
			.join('');
	}
	return splitSyllables(trimmed).join(DOT);
}

export const SYLLABLE_SEPARATOR = DOT;
