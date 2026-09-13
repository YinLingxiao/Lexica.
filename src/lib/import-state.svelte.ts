// Preserve the running import and its result when navigating between pages.
export const dictionaryImport = $state({
	sourcePath: '',
	running: false,
	message: '',
	error: ''
});
