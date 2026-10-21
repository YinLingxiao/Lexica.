// Minimal speech-synthesis controller. No UI, no state framework — callers just
// fire and forget. Repeated playback is queued with a short natural pause.

export interface SpeakOptions {
	accent?: 'US' | 'UK';
	rate?: number;
	repeat?: number;
}

type VoiceList = SpeechSynthesisVoice[];

let pauseTimer: ReturnType<typeof setTimeout> | null = null;
let session = 0;

function synth(): SpeechSynthesis | null {
	if (typeof window === 'undefined' || !('speechSynthesis' in window)) return null;
	return window.speechSynthesis;
}

function normaliseLang(voice: SpeechSynthesisVoice): string {
	return voice.lang.toLowerCase().replace('_', '-');
}

function pickVoice(voices: VoiceList, accent: 'US' | 'UK'): SpeechSynthesisVoice | null {
	const target = accent === 'US' ? 'en-us' : 'en-gb';
	const exact = voices.find((voice) => normaliseLang(voice) === target);
	if (exact) return exact;
	const regional = voices.find((voice) => normaliseLang(voice).startsWith(target));
	if (regional) return regional;
	return (
		voices.find((voice) => normaliseLang(voice).startsWith('en')) ??
		voices.find((voice) => normaliseLang(voice).startsWith('en-')) ??
		null
	);
}

function clearTimer(): void {
	if (pauseTimer !== null) {
		clearTimeout(pauseTimer);
		pauseTimer = null;
	}
}

/** Cancels any queued or active utterance. */
export function stopSpeaking(): void {
	session += 1;
	clearTimer();
	synth()?.cancel();
}

/**
 * Speaks text in the requested accent, rate and repeat count.
 * Repeats are separated by a 350 ms pause; a new call cancels the previous one.
 */
export function speakText(text: string, opts: SpeakOptions = {}): void {
	const content = typeof text === 'string' ? text.trim() : '';
	if (!content) return;
	const engine = synth();
	if (!engine) return;

	stopSpeaking();
	const run = session;
	const accent = opts.accent === 'US' ? 'US' : 'UK';
	const rate = opts.rate === 0.8 || opts.rate === 1.2 ? opts.rate : 1;
	const repeat = opts.repeat === 3 ? 3 : 1;
	const voices = engine.getVoices();
	const voice = pickVoice(voices, accent);

	let spoken = 0;

	const next = () => {
		if (run !== session) return;
		if (spoken >= repeat) return;
		spoken += 1;

		const utterance = new SpeechSynthesisUtterance(content);
		utterance.lang = accent === 'US' ? 'en-US' : 'en-GB';
		utterance.rate = rate;
		utterance.pitch = 1;
		utterance.volume = 1;
		if (voice) utterance.voice = voice;

		const scheduleNext = () => {
			if (run !== session || spoken >= repeat) return;
			clearTimer();
			pauseTimer = setTimeout(() => {
				pauseTimer = null;
				next();
			}, 350);
		};

		utterance.onend = scheduleNext;
		utterance.onerror = scheduleNext;

		try {
			// Chrome can leave the queue paused after a cancel on some platforms.
			engine.resume();
			engine.speak(utterance);
		} catch {
			/* Speech synthesis is best-effort. */
		}
	};

	next();
}
