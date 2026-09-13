import { browser } from '$app/environment';
type Theme = 'light' | 'dark' | 'system';
export const preferences = $state({
	theme: 'system' as Theme,
	reading: 'guided',
	fontSize: 'normal'
});
export function loadPreferences() {
	try {
		const value = JSON.parse(localStorage.getItem('lexica.preferences') ?? '{}');
		if (['light', 'dark', 'system'].includes(value.theme)) preferences.theme = value.theme;
		if (['guided', 'full'].includes(value.reading)) preferences.reading = value.reading;
		if (['normal', 'large'].includes(value.fontSize)) preferences.fontSize = value.fontSize;
	} catch {
		/* Use defaults if storage is unavailable. */
	}
}
export function applyPreferences() {
	if (!browser) return;
	document.documentElement.dataset.theme = preferences.theme;
	document.documentElement.dataset.fontSize = preferences.fontSize;
	try {
		localStorage.setItem('lexica.preferences', JSON.stringify(preferences));
	} catch {
		/* Applies for this session. */
	}
}
