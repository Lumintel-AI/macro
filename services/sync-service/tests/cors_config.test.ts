import type { Miniflare } from 'miniflare';
import { afterEach, describe, expect, test } from 'vitest';
import { getTokenForDocument, setupMiniflare } from './utils';

let mf: Miniflare | undefined;

afterEach(async () => {
  await mf?.dispose();
  mf = undefined;
});

describe('configured CORS origin', () => {
  test.each([undefined, '', 'https://app.example.com'])(
    'keeps defaults and only adds the exact configured origin %s',
    async (frontendOrigin) => {
      mf = await setupMiniflare({ frontendOrigin });
      const token = getTokenForDocument('cors-config-doc', 'test-user', 'owner');
      for (const origin of [
        'https://app.example.com',
        'https://unrelated.example.com',
        'https://app.example.com.attacker.test',
        'https://dev.macro.com',
        'http://persona.localhost:20010',
        'https://branch.preview.macro.com',
      ]) {
        const allowed = origin === frontendOrigin || [
          'https://dev.macro.com',
          'http://persona.localhost:20010',
          'https://branch.preview.macro.com',
        ].includes(origin);
        const preflight = await mf.dispatchFetch(
          'http://localhost:8787/document/cors-config-doc/metadata',
          {
            method: 'OPTIONS',
            headers: {
              Origin: origin,
              'Access-Control-Request-Method': 'GET',
              'Access-Control-Request-Headers': 'authorization,content-type,traceparent,tracestate',
            },
          },
        );
        expect(preflight.status).toBe(allowed ? 200 : 403);
        expect(preflight.headers.get('Access-Control-Allow-Origin')).toBe(allowed ? origin : null);
        if (allowed) {
          expect(preflight.headers.get('Access-Control-Allow-Credentials')).toBe('true');
          const headers = preflight.headers.get('Access-Control-Allow-Headers')!.toLowerCase();
          for (const header of ['authorization', 'content-type', 'traceparent', 'tracestate']) {
            expect(headers).toContain(header);
          }
        }
        const response = await mf.dispatchFetch(
          'http://localhost:8787/document/cors-config-doc/metadata',
          { headers: { Origin: origin, Authorization: `Bearer ${token}` } },
        );
        if (allowed) {
          expect([200, 404]).toContain(response.status);
          expect(response.headers.get('Access-Control-Allow-Origin')).toBe(origin);
        } else {
          expect(response.status).toBe(403);
          expect(response.headers.get('Access-Control-Allow-Origin')).toBeNull();
        }
      }
    },
  );

  test('configured origin still requires a signed document grant', async () => {
    mf = await setupMiniflare({ frontendOrigin: 'https://app.example.com' });
    const response = await mf.dispatchFetch(
      'http://localhost:8787/document/cors-config-doc/metadata',
      { headers: { Origin: 'https://app.example.com' } },
    );
    expect(response.status).toBe(401);
    expect(response.headers.get('Access-Control-Allow-Origin')).toBe('https://app.example.com');
  });
});
