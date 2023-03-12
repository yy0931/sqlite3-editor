import type { PlaywrightTestConfig } from '@playwright/test'
import { devices } from '@playwright/test'

export default {
  testDir: './tests',
  timeout: 30 * 1000, // Maximum time one test can run for.
  expect: {
    timeout: 15 * 1000
  },
  fullyParallel: false,  // Don't run tests in files in parallel
  forbidOnly: !!process.env.CI,  // Fail the build on CI if you accidentally left test.only in the source code.
  retries: process.env.CI ? 2 : 0,  // Retry on CI only
  workers: process.env.CI ? 1 : undefined,  // Opt out of parallel tests on CI.
  reporter: 'html',
  use: {
    actionTimeout: 0,
    trace: 'on-first-retry',
  },
  projects: [
    // TODO: Since the database file is shared by the server, it is not possible to run the same test on multiple browsers at the same time.
    {
      name: 'chromium',
      use: {
        ...devices['Desktop Chrome'],
      },
    },
  ],
} satisfies PlaywrightTestConfig
