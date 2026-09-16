#!/usr/bin/env node
// 本地发布脚本：构建产物 → 创建/更新 GitHub Release → 上传安装包。
//
// 用法：
//   node scripts/publish-release.mjs                 # 构建 + 发布当前版本
//   node scripts/publish-release.mjs --skip-build    # 复用已有产物
//   node scripts/publish-release.mjs --replace       # 覆盖同名资源
//   node scripts/publish-release.mjs --tag v0.2.0    # 指定 tag（默认 v<版本>）
//   node scripts/publish-release.mjs --notes my.md   # 指定说明文件
//
// 凭据：优先读环境变量 GITHUB_TOKEN；没有则从 Git 凭据管理器读取。
//
// 为什么不用 GitHub Actions：本仓库名以点结尾（Lexica.），而 Windows runner 的
// 工作目录是 D:\a\<仓库名>\<仓库名>。Windows 无法创建以点结尾的目录，checkout
// 会直接报 "Directory 'D:\a\Lexica.\Lexica.' does not exist"。这是 runner 工作
// 目录的硬限制，与 workflow 写法无关。

import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, statSync } from 'node:fs';

const VERSION_FROM = 'src-tauri/tauri.conf.json';
const DEFAULT_NOTES = 'docs/release-notes-template.md';
const STABLE_ALIASES = {
	nsis: 'Lexica-setup.exe',
	msi: 'Lexica.msi'
};

// ---------------------------------------------------------------- 参数

const argv = process.argv.slice(2);
const flag = (name) => argv.includes(name);
const value = (name, fallback = null) => {
	const i = argv.indexOf(name);
	return i !== -1 && argv[i + 1] ? argv[i + 1] : fallback;
};

const skipBuild = flag('--skip-build');
const replace = flag('--replace');
const notesFile = value('--notes', DEFAULT_NOTES);

// ---------------------------------------------------------------- 环境

function git(args, input) {
	return execFileSync('git', args, {
		input,
		encoding: 'utf8',
		stdio: input === undefined ? ['ignore', 'pipe', 'ignore'] : ['pipe', 'pipe', 'ignore']
	}).trim();
}

function resolveRepo() {
	const url = git(['remote', 'get-url', 'origin']);
	const match = /github\.com[/:]([^/]+)\/(.+?)(?:\.git)?$/.exec(url);
	if (!match) throw new Error(`无法从 origin 解析仓库：${url}`);
	return { owner: match[1], repo: match[2] };
}

function resolveToken() {
	if (process.env.GITHUB_TOKEN) return process.env.GITHUB_TOKEN;
	try {
		const out = git(['credential', 'fill'], 'protocol=https\nhost=github.com\n\n');
		const match = /^password=(.*)$/m.exec(out);
		if (match) return match[1].trim();
	} catch {
		/* 凭据管理器不可用 */
	}
	throw new Error('未找到凭据。请设置 GITHUB_TOKEN，或在 Git 凭据管理器里登录 GitHub。');
}

const { owner, repo } = resolveRepo();
const version = JSON.parse(readFileSync(VERSION_FROM, 'utf8')).version;
const tag = value('--tag', `v${version}`);
const token = resolveToken();

const API = `https://api.github.com/repos/${owner}/${repo}`;
const UPLOADS = `https://uploads.github.com/repos/${owner}/${repo}`;
const HEADERS = {
	Authorization: `Bearer ${token}`,
	Accept: 'application/vnd.github+json',
	'X-GitHub-Api-Version': '2022-11-28',
	'User-Agent': 'lexica-publish'
};

async function api(path, init = {}) {
	const res = await fetch(`${API}${path}`, {
		...init,
		headers: { ...HEADERS, ...(init.headers ?? {}) }
	});
	const text = await res.text();
	let json = null;
	try {
		json = text ? JSON.parse(text) : null;
	} catch {
		/* 非 JSON */
	}
	return { ok: res.ok, status: res.status, json, text };
}

// ---------------------------------------------------------------- 流程

function build() {
	console.log('▶ 构建安装包（npm run tauri build）…');
	execFileSync('npm', ['run', 'tauri', 'build'], { stdio: 'inherit', shell: true });
}

function bundlePaths() {
	return [
		{
			kind: 'nsis',
			file: `src-tauri/target/release/bundle/nsis/Lexica_${version}_x64-setup.exe`
		},
		{
			kind: 'msi',
			file: `src-tauri/target/release/bundle/msi/Lexica_${version}_x64_en-US.msi`
		}
	];
}

function renderNotes() {
	if (!existsSync(notesFile)) {
		console.warn(`⚠ 未找到说明文件 ${notesFile}，Release 说明留空。`);
		return '';
	}
	return readFileSync(notesFile, 'utf8')
		.replaceAll('{{VERSION}}', version)
		.replaceAll('{{TAG}}', tag);
}

async function findRelease() {
	// 注意：/releases/tags/<tag> 对以点结尾的仓库名会 404，只能列出来自己筛。
	const res = await api('/releases?per_page=100');
	if (!res.ok) throw new Error(`读取 releases 失败：${res.status} ${res.text.slice(0, 200)}`);
	return (res.json ?? []).find((r) => r.tag_name === tag) ?? null;
}

async function main() {
	if (!skipBuild) build();

	const missing = bundlePaths().filter((b) => !existsSync(b.file));
	if (missing.length) {
		console.error('✖ 找不到构建产物，请先运行 npm run tauri build：');
		for (const b of missing) console.error(`    ${b.file}`);
		process.exit(1);
	}

	const body = renderNotes();
	let release = await findRelease();

	if (release) {
		console.log(`▶ 更新已存在的 Release ${tag}（#${release.id}）`);
		const res = await api(`/releases/${release.id}`, {
			method: 'PATCH',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ name: `Lexica ${tag}`, body })
		});
		if (!res.ok) throw new Error(`更新失败：${res.status} ${res.text.slice(0, 200)}`);
		release = res.json;
	} else {
		console.log(`▶ 创建 Release ${tag}`);
		const res = await api('/releases', {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({
				tag_name: tag,
				target_commitish: 'main',
				name: `Lexica ${tag}`,
				body,
				draft: false,
				prerelease: false
			})
		});
		if (!res.ok) throw new Error(`创建失败：${res.status} ${res.text.slice(0, 200)}`);
		release = res.json;
	}

	console.log(`  ${release.html_url}`);

	// 每个产物上传两份：带版本号（便于确认版本）+ 固定名（永久链接）
	const existing = new Map((release.assets ?? []).map((a) => [a.name, a.id]));

	for (const { kind, file } of bundlePaths()) {
		const buf = readFileSync(file);
		const size = Math.round(statSync(file).size / 1024);
		const names = [file.split(/[/\\]/).pop(), STABLE_ALIASES[kind]];

		for (const name of names) {
			const id = existing.get(name);
			if (id && !replace) {
				console.log(`  ⏭ 已存在，跳过  ${name}`);
				continue;
			}
			if (id) {
				await api(`/releases/assets/${id}`, { method: 'DELETE' });
				console.log(`  ↻ 已删除旧资源  ${name}`);
			}
			const res = await fetch(
				`${UPLOADS}/releases/${release.id}/assets?name=${encodeURIComponent(name)}`,
				{
					method: 'POST',
					headers: { ...HEADERS, 'Content-Type': 'application/octet-stream' },
					body: buf
				}
			);
			if (!res.ok) {
				console.error(`  ✖ 上传失败 ${name}：${res.status} ${(await res.text()).slice(0, 200)}`);
				continue;
			}
			console.log(`  ✅ ${name}  (${size} KB)`);
		}
	}

	const final = await api(`/releases/${release.id}`);
	console.log('\n=== Release 资源 ===');
	for (const a of final.json.assets ?? []) {
		console.log(`  ${String(Math.round(a.size / 1024)).padStart(6)} KB  ${a.name}`);
	}
	console.log('\n永久链接（始终指向最新版本）：');
	for (const name of Object.values(STABLE_ALIASES)) {
		console.log(`  https://github.com/${owner}/${repo}/releases/latest/download/${name}`);
	}
}

await main();
process.exit(0);
