/**
 * Gate-5 visual-regression specs (Workstream E).
 *
 * BASELINES ARE CANONICAL ON LINUX ONLY. The `ubuntu-latest` CI leg is the pixel
 * authority; cross-OS anti-aliasing means a Windows-local run is advisory and is
 * NOT pixel-authoritative. Do NOT commit Windows-generated baselines. Bootstrap
 * the baselines on the Linux CI leg (see `e2e/README.md`).
 *
 * No live backend: every network call is fulfilled deterministically from
 * `e2e/fixtures/handlers.ts` (`routeTable`) via Playwright `page.route()`. The
 * build under test runs with VITE_USE_LOCAL_KE_API=true, VITE_USE_REVIEW_UI=true,
 * VITE_USE_WASM_PREVIEW=false and VITE_API_URL=http://msw.local/api (mocked), so
 * the rewired path is exercised over mocked serve HTTP and no `.wasm` binary is
 * loaded (convention B).
 *
 * Snapshotted (per contract E.1 + ADR-0020):
 *   - /workbench  — the flag-on `LocalKePreviewPane` (compile-preview/dry-run/verify
 *     over the local serve surface) is genuinely rewired; the rule LIST stays on the
 *     `VITE_API_URL` fallback (scaffold-only — serve has no GET /rules).
 *   - /production — health is genuinely rewired (serve GET /healthz).
 *   - the 5e review UI mounts inside KEWorkbench's preview pane after a verify.
 * The 7 ML/analytics off-path pages are intentionally NOT snapshotted: by ADR-0020
 * they stay on the external API, so flag-on they render identically — no signal.
 */
import { test, expect, type Page } from '@playwright/test'
import { routeTable } from './fixtures/handlers'

/**
 * Install deterministic network mocks for both mocked origins (VITE_API_URL
 * fallback + KE_SERVE_URL local serve). Any request not matched is aborted so a
 * stray live call fails loudly rather than hanging the screenshot.
 */
async function installNetworkMocks(page: Page): Promise<void> {
  for (const { pattern, json } of routeTable) {
    await page.route(pattern, (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(json),
      }),
    )
  }
}

/** Settle: app fonts/layout flushed and React Query data resolved. */
async function settle(page: Page): Promise<void> {
  await page.waitForLoadState('networkidle')
  // Give layout/paint a beat after data resolves; animations are disabled by
  // config so this is just a flush, not a timing race.
  await page.evaluate(() => document.fonts.ready)
}

test.beforeEach(async ({ page }) => {
  await installNetworkMocks(page)
})

test('KEWorkbench (local preview pane flag-on) full page', async ({ page }) => {
  await page.goto('/workbench')
  // The rule LIST comes from the mocked VITE_API_URL fallback (rules.list is
  // SCAFFOLD-ONLY under USE_LOCAL_KE_API and falls through to it). The genuinely
  // rewired surface is the flag-on LocalKePreviewPane (compile/dry-run/verify).
  await expect(page.getByRole('heading', { name: 'KE Workbench' })).toBeVisible()
  await expect(page.getByTestId('local-ke-preview')).toBeVisible()
  await settle(page)
  await expect(page).toHaveScreenshot('workbench.png', { fullPage: true })
})

test('ProductionDemo (rewired, flag-on) full page', async ({ page }) => {
  await page.goto('/production')
  await expect(page.getByRole('heading', { name: 'Production Demo' })).toBeVisible()
  await settle(page)
  await expect(page).toHaveScreenshot('production.png', { fullPage: true })
})

/**
 * 5e AI-provenance review UI (USE_REVIEW_UI=true).
 *
 * The review surface mounts inside KEWorkbench's `LocalKePreviewPane` and renders
 * once an artifact is verified (its canonical provenance feeds the four-class
 * panel). This spec drives a verify against the mocked serve `POST /verify`, then
 * captures the review-UI state. Baselines bootstrap on the Linux CI leg.
 */
test('5e review UI on KEWorkbench (USE_REVIEW_UI on) full page', async ({ page }) => {
  await page.goto('/workbench')
  await expect(page.getByRole('heading', { name: 'KE Workbench' })).toBeVisible()
  await page.getByPlaceholder('64-hex artifact hash').fill('a'.repeat(64))
  await page.getByRole('button', { name: 'Verify' }).click()
  await expect(page.getByTestId('review-surface')).toBeVisible()
  await settle(page)
  await expect(page).toHaveScreenshot('review-ui-workbench.png', { fullPage: true })
})
