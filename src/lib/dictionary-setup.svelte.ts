import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import {
	appInfo,
	ensureDictionary,
	errorMessage,
	isPreview,
	type EnsureDictionaryResult,
	type SetupProgress
} from './api';

export const dictionarySetup = $state({
	active: false,
	phase: 'checking' as SetupProgress['phase'],
	progress: null as number | null,
	message: '',
	error: '',
	result: null as EnsureDictionaryResult | null
});

let running: Promise<EnsureDictionaryResult> | null = null;

export async function runDictionarySetup(force = false): Promise<EnsureDictionaryResult | null> {
	if (isPreview()) return null;
	if (running) return running;
	if (!force) {
		try {
			const info = await appInfo();
			if (info.dictionary_ready) {
				return {
					ready: true,
					word_count: info.word_count,
					imported: 0,
					skipped: true
				};
			}
		} catch {
			/* Fall through to full ensure. */
		}
	}
	dictionarySetup.active = true;
	dictionarySetup.error = '';
	dictionarySetup.phase = 'checking';
	dictionarySetup.progress = null;
	dictionarySetup.message = 'Checking dictionary…';
	let unlisten: UnlistenFn | undefined;
	running = (async () => {
		try {
			unlisten = await listen<SetupProgress>('dictionary-setup', (event) => {
				const p = event.payload;
				dictionarySetup.phase = p.phase;
				dictionarySetup.progress = p.progress;
				dictionarySetup.message = p.message;
				if (p.phase === 'error') dictionarySetup.error = p.message;
			});
			const result = await ensureDictionary();
			dictionarySetup.result = result;
			dictionarySetup.phase = 'ready';
			dictionarySetup.progress = 1;
			dictionarySetup.message = `Dictionary ready · ${result.word_count.toLocaleString()} entries`;
			dictionarySetup.active = false;
			return result;
		} catch (e) {
			dictionarySetup.phase = 'error';
			dictionarySetup.error = errorMessage(e);
			dictionarySetup.message = dictionarySetup.error;
			dictionarySetup.active = true;
			throw e;
		} finally {
			unlisten?.();
			running = null;
		}
	})();
	return running;
}
