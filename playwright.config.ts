import { defineConfig } from '@playwright/test';
export default defineConfig({
	testDir: './tests',
	fullyParallel: true,
	workers: 2,
	use: {
		baseURL: 'http://127.0.0.1:5173',
		viewport: { width: 1180, height: 820 },
		colorScheme: 'light',
		screenshot: 'only-on-failure',
		trace: 'retain-on-failure'
	},
	webServer: { command: 'npm run dev', url: 'http://127.0.0.1:5173', reuseExistingServer: true },
	reporter: 'list'
});
