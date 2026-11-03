import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { runInNewContext } from 'node:vm';
import { build, type Rollup } from 'vite';
import { afterEach, expect, it, vi } from 'vitest';
import { createAppViteConfig } from '../vite.base';

const root = fileURLToPath(new URL('..', import.meta.url)).replace(/\/$/, '');

afterEach(() => vi.unstubAllEnvs());

async function productionConfig() {
  vi.stubEnv('MODE', 'production');
  vi.stubEnv('NODE_ENV', 'production');
  return {
    ...(await createAppViteConfig()({ command: 'build', mode: 'production' })),
    configFile: false as const,
    root,
    logLevel: 'silent' as const,
  };
}

async function compileHosts() {
  const config = await productionConfig();
  const result = (await build({
    ...config,
    publicDir: false,
    plugins: [
      config.plugins,
      {
        name: 'host-contract-entry',
        enforce: 'pre',
        load(id) {
          // Compile the real HTML and host module without unrelated app/WASM code.
          if (id === `${root}/src/index.tsx`) {
            return `import { SERVER_HOSTS, SYNC_SERVICE_HOSTS, staticFileReferenceEndpoint } from './lib/core/constant/servers';
              globalThis.hostContract = { servers: SERVER_HOSTS, sync: SYNC_SERVICE_HOSTS, staticReference: staticFileReferenceEndpoint('fixture') };`;
          }
        },
      },
    ],
    build: {
      ...config.build,
      write: false,
      modulePreload: false,
      rollupOptions: {
        input: `${root}/index.html`,
      },
    },
  })) as Rollup.RollupOutput;
  const chunk = result.output.find(
    (file) => file.type === 'chunk' && file.isEntry
  );
  const html = result.output.find((file) => file.fileName === 'index.html');
  if (chunk?.type !== 'chunk' || html?.type !== 'asset') {
    throw new Error('Expected production JavaScript and HTML');
  }
  const context = {
    hostContract: {
      servers: {} as Record<string, string>,
      sync: { worker: '', ws: '' },
      staticReference: '',
    },
  };
  runInNewContext(chunk.code, context);
  return {
    ...context.hostContract,
    code: chunk.code,
    html: String(html.source),
  };
}

it('emits configured production APIs, WebSockets, identity and local HTML assets', async () => {
  vi.stubEnv('VITE_PUBLIC_ORIGIN', 'https://app.example.test/');
  vi.stubEnv(
    'VITE_FUSIONAUTH_CLIENT_ID',
    '00000000-0000-4000-8000-000000000001'
  );
  vi.stubEnv(
    'VITE_FUSIONAUTH_TENANT_ID',
    '00000000-0000-4000-8000-000000000002'
  );
  vi.stubEnv('VITE_SYNC_SERVICE_HOST', 'app.example.test/sync');
  const result = await compileHosts();
  for (const host of Object.values(result.servers)) {
    expect(new URL(host).hostname).toBe('app.example.test');
  }
  expect(result.servers['connection-gateway']).toBe(
    'wss://app.example.test/connection-gateway'
  );
  for (const name of ['agent-harness', 'scheduled-action', 'preview']) {
    expect(result.servers[name]).toBe(`https://app.example.test/${name}`);
  }
  expect(result.servers['cognition-service']).toBe(
    'https://app.example.test/cognition'
  );
  expect(result.servers['document-storage-service']).toBe(
    'https://app.example.test/dss'
  );
  expect(result.servers['pdf-service']).toBe('https://app.example.test/i');
  expect(result.servers['websocket-service']).toBe(
    'wss://app.example.test/websocket'
  );
  expect(result.staticReference).toBe(
    'https://app.example.test/static-file/file/fixture'
  );
  const logout = new URL(result.servers['auth-logout']);
  expect(logout.pathname).toBe('/oauth2/logout');
  expect(logout.searchParams.get('client_id')).toBe(
    '00000000-0000-4000-8000-000000000001'
  );
  expect(logout.searchParams.get('tenantId')).toBe(
    '00000000-0000-4000-8000-000000000002'
  );
  expect(result.sync).toEqual({
    worker: 'https://app.example.test/sync',
    ws: 'wss://app.example.test/sync',
  });
  expect(result.code).not.toContain('macro.com');
  expect(result.html).not.toMatch(/%[A-Z_]+%/);
  expect(result.html).not.toContain('macro.com');
  expect(result.html).toContain('href="https://app.example.test"');
  for (const asset of ['macro-favicon.svg', 'logo192.png', 'manifest.json']) {
    expect(result.html).toContain(`href="/${asset}"`);
    expect(existsSync(`${root}/public/${asset}`)).toBe(true);
  }
}, 30_000);

it('preserves upstream production defaults and allows a direct sync Worker', async () => {
  vi.stubEnv('VITE_PUBLIC_ORIGIN', '');
  vi.stubEnv('VITE_SYNC_SERVICE_HOST', 'sync.example.workers.dev');
  const result = await compileHosts();
  expect(result.servers['auth-service']).toBe('https://gateway.macro.com/auth');
  expect(result.servers['auth-logout']).toBe(
    'https://auth.macro.com/oauth2/logout?client_id=75409999-7dc4-4241-b73b-a51818c3a71c&tenantId=a3e53c3d-8d6a-3e92-d64c-fa3bf30a60be'
  );
  expect(result.servers['static-file']).toBe(
    'https://static-file-service.macro.com'
  );
  expect(result.sync.worker).toBe('https://sync.example.workers.dev');
  expect(result.html).toContain('href="https://gateway.macro.com"');
}, 30_000);

it('rejects a self-hosted build with missing identity instead of using upstream logout', async () => {
  vi.stubEnv('VITE_PUBLIC_ORIGIN', 'https://app.example.test');
  vi.stubEnv('VITE_FUSIONAUTH_CLIENT_ID', '');
  await expect(productionConfig()).rejects.toThrow('VITE_FUSIONAUTH_CLIENT_ID');
});

it('rejects a public URL containing a path', async () => {
  vi.stubEnv('VITE_PUBLIC_ORIGIN', 'https://app.example.test/app');
  await expect(productionConfig()).rejects.toThrow('VITE_PUBLIC_ORIGIN');
});

it('keeps sync on the configured origin when no Worker override is supplied', async () => {
  vi.stubEnv('VITE_PUBLIC_ORIGIN', 'https://app.example.test');
  vi.stubEnv('VITE_FUSIONAUTH_CLIENT_ID', '00000000-0000-4000-8000-000000000001');
  vi.stubEnv('VITE_FUSIONAUTH_TENANT_ID', '00000000-0000-4000-8000-000000000002');
  vi.stubEnv('VITE_SYNC_SERVICE_HOST', '');
  const result = await compileHosts();
  expect(result.sync).toEqual({
    worker: 'https://app.example.test/sync',
    ws: 'wss://app.example.test/sync',
  });
  expect(result.code).not.toContain('macroverse.workers.dev');
}, 30_000);
