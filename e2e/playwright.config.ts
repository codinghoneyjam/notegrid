import { defineConfig, devices } from '@playwright/test';

const PORT = Number(process.env.E2E_PORT ?? 4318);

export default defineConfig({
  testDir: './tests',
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: 0,
  reporter: [['list']],
  use: {
    baseURL: `http://127.0.0.1:${PORT}`,
    trace: 'off',
  },
  projects: [
    {
      name: 'chromium',
      use: {
        ...devices['Desktop Chrome'],
        // The scenario plays audio; a headless run must not sit suspended
        // on the autoplay policy while the spec waits for blocks.
        launchOptions: { args: ['--autoplay-policy=no-user-gesture-required'] },
      },
    },
    {
      name: 'firefox',
      use: {
        ...devices['Desktop Firefox'],
        launchOptions: { firefoxUserPrefs: { 'media.autoplay.default': 0 } },
      },
    },
    { name: 'webkit', use: { ...devices['Desktop Safari'] } },
  ],
  // The harness bundles the TS sources and serves the wasm, so it must be up first.
  webServer: {
    command: 'node build.mjs',
    url: `http://127.0.0.1:${PORT}/harness.html`,
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});