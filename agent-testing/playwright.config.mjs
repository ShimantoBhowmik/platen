// Playwright projects for the end-to-end run. Start it with `node run.mjs`, which brings up
// the stack and passes its address in BASE_URL.

import { defineConfig } from "@playwright/test";

if (!process.env.BASE_URL) {
    throw new Error("BASE_URL is not set. Run the checks with `node run.mjs`.");
}

export default defineConfig({
    testDir: "tests",
    outputDir: "results/artifacts",
    // A typed test takes real time: the server rejects results that arrive faster than they
    // could have been typed.
    timeout: 90_000,
    expect: { timeout: 10_000 },
    fullyParallel: true,
    workers: 4,
    forbidOnly: true,
    // A flaky check is a bug to fix, not to retry.
    retries: 0,
    reporter: [["list"], ["json", { outputFile: "results/report.json" }]],
    use: {
        baseURL: process.env.BASE_URL,
        // Playwright emulates a light system theme unless told otherwise. Dark is the app's
        // primary theme; the checks switch to light themselves.
        colorScheme: "dark",
        trace: "retain-on-failure",
        screenshot: "only-on-failure",
    },
    projects: [
        { name: "api", testMatch: /api\.spec\.mjs/ },
        {
            name: "desktop",
            testMatch: /ui\.spec\.mjs/,
            use: { browserName: "chromium", viewport: { width: 1440, height: 900 } },
        },
        {
            name: "laptop",
            testMatch: /ui\.spec\.mjs/,
            use: { browserName: "chromium", viewport: { width: 1280, height: 720 } },
        },
        {
            name: "phone",
            testMatch: /ui\.spec\.mjs/,
            use: {
                browserName: "chromium",
                viewport: { width: 390, height: 844 },
                deviceScaleFactor: 3,
                isMobile: true,
                hasTouch: true,
            },
        },
    ],
});
