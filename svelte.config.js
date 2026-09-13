import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess(),
	kit: {
		// Tauri 以自定义协议服务静态文件。SPA 模式：所有路由 fallback 到 index.html。
		adapter: adapter({
			fallback: 'index.html'
		})
	}
};

export default config;
