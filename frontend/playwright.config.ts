/**
 * Gate-5 self-hosted visual-regression harness (Workstream E).
 *
 * BASELINE AUTHORITY: baselines are CANONICAL ON LINUX (the `ubuntu-latest` CI
 * leg). Cross-OS font hinting / anti-aliasing differs enough that a
 * Windows-generated PNG will not match a Linux baseline pixel-for-pixel. Local
 * Windows runs are therefore ADVISORY ONLY and are NOT pixel-authoritative — do
 * not commit Windows-generated baselines as canonical. The single Playwright
 * project is named `linux`; `snapshotPathTemplate` bakes that into the committed
 * baseline filenames so only the Linux leg writes/verifies them. First-run
 * baseline bootstrap is documented in `e2e/README.md`.
 *
 * No live backend: the spec mocks every network call deterministically (see
 * `e2e/fixtures/handlers.ts`). The visual build runs with
 * `VITE_USE_LOCAL_KE_API=true VITE_USE_REVIEW_UI=true VITE_USE_WASM_PREVIEW=false`
 * so the rewired serve-HTTP path is exercised while the WASM `.wasm` binary is
 * never loaded (convention B). `VITE_API_URL` is baked to `http://msw.local/api`
 * (mocked), so even the SCAFFOLD-ONLY fallback path renders without a server.
 */
import { defineConfig, devices } from '@playwright/test'

const PORT = 4173
const BASE_URL = `http://localhost:${PORT}`

export default defineConfig({
  testDir: './e2e',
  // The vitest unit suite lives under src/**; keep these disjoint.
  testMatch: '**/*.spec.ts',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: process.env.CI ? [['github'], ['html', { open: 'never' }]] : [['html', { open: 'never' }]],

  // Linux is the pixel authority: project name `linux` is baked into baseline
  // filenames so a Windows-local run can never overwrite the canonical PNGs.
  snapshotPathTemplate: 'e2e/__screenshots__/{testFilePath}/{arg}-{projectName}.png',

  expect: {
    toHaveScreenshot: {
      // Small tolerance for sub-pixel jitter; cross-OS drift is handled by the
      // Linux-only baseline policy, not by loosening this.
      maxDiffPixelRatio: 0.01,
      animations: 'disabled',
    },
  },

  use: {
    baseURL: BASE_URL,
    // Pinned deterministic render surface (see contract E.1).
    viewport: { width: 1280, height: 800 },
    deviceScaleFactor: 1,
    colorScheme: 'light',
    trace: 'on-first-retry',
  },

  projects: [
    {
      name: 'linux',
      use: { ...devices['Desktop Chrome'], channel: undefined },
    },
  ],

  webServer: {
    command: `npm run preview -- --port ${PORT} --strictPort`,
    url: BASE_URL,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
})
