import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [sveltekit()],
	// Tauri 推荐配置：保留编译错误输出、固定端口
	clearScreen: false,
	server: {
		host: '127.0.0.1',
		port: 5173,
		strictPort: true
	},
	envPrefix: ['VITE_', 'TAURI_ENV_']
});
