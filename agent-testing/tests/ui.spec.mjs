// The app in a real browser: typing with the keyboard, the countdown, the server-verified
// result plate, preferences and error states. Runs at desktop, laptop and phone sizes.

import {
    expect,
    keysWithMistakes,
    pageSize,
    screen,
    test,
    typeLikeHuman,
} from "./support.mjs";

/** Waits for the app's first sheet. */
async function openApp(page) {
    await page.goto("/");
    await expect(page.locator("[data-corner=readout]")).toHaveText("ready", { timeout: 30_000 });
}

/** Picks a test length on the rail and returns the words the server issued for it. */
async function chooseMode(page, label) {
    const issued = page.waitForResponse(
        (response) => response.url().endsWith("/api/tests") && response.request().method() === "POST",
    );
    await page.getByRole("button", { name: label, exact: true }).click();
    const { words } = await (await issued).json();
    await expect(page.locator("[data-corner=readout]")).toHaveText("ready");
    return words;
}

/** The next result the app submits, as the server answered it. */
function nextResult(page) {
    return page.waitForResponse(
        (response) =>
            response.url().endsWith("/api/results") && response.request().method() === "POST",
        { timeout: 40_000 },
    );
}

/** The value of the result's stat with exactly this label, e.g. `"keys"`. */
function stat(plate, label) {
    return plate
        .locator(".tw-stats > div")
        .filter({ has: plate.page().locator("dt", { hasText: new RegExp(`^${label}$`) }) })
        .locator("dd")
        .first();
}

/** Fails if the page scrolls sideways, or on a desktop size, at all. */
async function expectFits(page, testInfo) {
    const size = await pageSize(page);
    expect(size.scrollWidth, "no horizontal scroll").toBeLessThanOrEqual(size.width);
    if (testInfo.project.name !== "phone") {
        expect(size.scrollHeight, "the page fits the window").toBeLessThanOrEqual(size.height);
    }
}

test("a word test typed on the keyboard ends in the verified result plate", async ({ page }, testInfo) => {
    await openApp(page);
    await expect(page).toHaveTitle("platen");
    const words = await chooseMode(page, "10");
    const count = page.locator(".tw-count");
    await expect(count).toHaveText("0/10");
    await expect(page.locator("[data-corner=fig]")).toHaveText("Fig 1");
    await expectFits(page, testInfo);
    await screen(page, testInfo, "1-ready");

    // Type the first word and its space, then check the live state.
    await page.locator("#tw-stage").click();
    const keys = keysWithMistakes(words);
    const firstWord = `${words[0]} `;
    const scored = nextResult(page);
    await typeLikeHuman(page, firstWord);
    await expect(count).toHaveText("1/10");
    await expect(page.locator("[data-corner=readout]")).toHaveText("typing");
    await screen(page, testInfo, "2-typing");
    await typeLikeHuman(page, keys.slice(firstWord.length), 11);

    const response = await scored;
    expect(response.status()).toBe(201);
    const { stats, breakdown, timeline } = await response.json();

    // The plate shows exactly what the server verified.
    const plate = page.locator(".tw-result");
    await expect(plate).toBeVisible();
    await expect(count).toHaveCount(0);
    await expect(page.locator("[data-corner=fig]")).toHaveText("Fig 2");
    await expect(page.locator("[data-corner=readout]")).toHaveText("verified by the server");
    const [wpmShown, accuracyShown] = await plate.locator(".tw-figures dd").allTextContents();
    expect(Math.abs(Number.parseInt(wpmShown, 10) - stats.wpm)).toBeLessThanOrEqual(0.5);
    expect(Math.abs(Number.parseInt(accuracyShown, 10) - stats.accuracy)).toBeLessThanOrEqual(0.5);
    await expect(stat(plate, "keys")).toHaveText(
        `${stats.correct_keystrokes}/${stats.incorrect_keystrokes}`,
    );
    await expect(stat(plate, "test")).toHaveText("words 10");
    await expect(plate.locator(".tw-stats .tw-sub").first()).toHaveText("english 1k");
    await expect(page.getByRole("status")).toContainText("Test scored.");

    // The chart draws one point per second and a mark for each second with wrong keys.
    const chart = plate.locator(".tw-chart-svg");
    await expect(chart).toHaveAttribute("aria-label", /^Speed over \d+ seconds?:/);
    await expect(chart.locator(".tw-dot")).toHaveCount(timeline.length);
    await expect(chart.locator(".tw-err")).toHaveCount(timeline.filter((p) => p.errors > 0).length);
    const middle = chart.locator(".tw-column").nth(Math.floor(timeline.length / 2));
    if (testInfo.project.name === "phone") await middle.click();
    else await middle.hover();
    await expect(plate.locator(".tw-tip")).toHaveText(/^second \d+: \d+ wpm, raw \d+/);

    // What to practise: the word left wrong and the missed keys.
    await expect(plate.locator(".tw-words li").first()).toContainText(words[2]);
    const missedKeys = breakdown.keys.filter((key) => key.misses > 0).length;
    await expect(plate.locator(".tw-keycaps li")).toHaveCount(Math.min(6, missedKeys));
    await expectFits(page, testInfo);
    await screen(page, testInfo, "3-result");

    if (testInfo.project.name === "phone") {
        // A rail chip must not jump the page, even when the stage is off screen.
        await page.evaluate(() => {
            document.activeElement?.blur();
            scrollTo(0, 0);
        });
        await page.getByRole("button", { name: "Light theme" }).click();
        await expect(page.locator(".tw-page")).toHaveAttribute("data-theme", "light");
        expect((await pageSize(page)).scrollY).toBe(0);
        await screen(page, testInfo, "4-result-light");
    }

    // Escape feeds a fresh sheet.
    await page.locator("#tw-stage").focus();
    await page.keyboard.press("Escape");
    await expect(page.locator("[data-corner=readout]")).toHaveText("ready");
    await expect(page.locator("[data-corner=fig]")).toHaveText("Fig 1");
    await expect(count).toHaveText("0/10");
});

test("a time test counts down and is scored at its limit", async ({ page }, testInfo) => {
    test.skip(testInfo.project.name !== "desktop", "one size is enough for a 15 s wait");
    await openApp(page);
    const words = await chooseMode(page, "15s");
    const count = page.locator(".tw-count");
    await expect(count).toHaveText("15");

    await page.locator("#tw-stage").click();
    const scored = nextResult(page);
    await typeLikeHuman(page, `${words.slice(0, 5).join(" ")} `);
    const left = Number.parseInt(await count.textContent(), 10);
    expect(left).toBeGreaterThan(0);
    expect(left).toBeLessThan(15);

    const response = await scored;
    expect(response.status()).toBe(201);
    const { timeline } = await response.json();
    expect(timeline).toHaveLength(15);
    const plate = page.locator(".tw-result");
    await expect(plate.locator(".tw-dot")).toHaveCount(15);
    await expect(stat(plate, "time")).toHaveText("15s");
    await screen(page, testInfo, "5-time-result");
});

test("theme and sound choices survive a reload", async ({ page }) => {
    await openApp(page);
    const theme = page.getByRole("button", { name: "Light theme" });
    const sound = page.getByRole("button", { name: "Typewriter sound" });
    await expect(sound).toHaveAttribute("aria-pressed", "true");

    await theme.click();
    await sound.click();
    await expect(page.locator(".tw-page")).toHaveAttribute("data-theme", "light");
    await page.reload();
    await expect(page.locator("[data-corner=readout]")).toHaveText("ready", { timeout: 30_000 });

    await expect(page.locator(".tw-page")).toHaveAttribute("data-theme", "light");
    await expect(theme).toHaveAttribute("aria-pressed", "true");
    await expect(sound).toHaveAttribute("aria-pressed", "false");
});

test("typing plays the recorded typewriter clips", async ({ page }, testInfo) => {
    test.skip(testInfo.project.name !== "desktop", "sound does not depend on size");
    // Records every buffer the app starts: its length and one sample, which tells clips apart.
    await page.addInitScript(() => {
        window.played = [];
        const start = AudioBufferSourceNode.prototype.start;
        AudioBufferSourceNode.prototype.start = function (...args) {
            window.played.push({
                seconds: this.buffer.duration,
                sample: this.buffer.getChannelData(0)[200],
            });
            return start.apply(this, args);
        };
    });
    await openApp(page);
    const words = await chooseMode(page, "10");

    const keys = `${words[0]} ${words[1][0]}`;
    await typeLikeHuman(page, keys);
    await page.keyboard.press("Backspace");

    await expect.poll(() => page.evaluate(() => window.played.length)).toBe(keys.length + 1);
    const played = await page.evaluate(() => window.played);
    const backspace = played.pop();
    // Clacks and the space bar are 160 ms cuts of the recording; the backspace is a lighter
    // 120 ms tick, not the space bar's thump.
    for (const clip of played) expect(clip.seconds).toBeCloseTo(0.16, 3);
    expect(backspace.seconds).toBeCloseTo(0.12, 3);
    const clacks = new Set(played.slice(0, words[0].length).map((clip) => clip.sample));
    expect(clacks.size, "consecutive keys use different clack variants").toBe(
        Math.min(words[0].length, 6),
    );
});

test.describe("error states", () => {
    test.use({ allowedConsoleErrors: [/Failed to load resource/] });

    test("a refused result says why and how to recover", async ({ page }, testInfo) => {
        test.skip(testInfo.project.name !== "desktop", "error copy does not depend on size");
        await page.route("**/api/results", (route) =>
            route.fulfill({
                status: 422,
                contentType: "application/json",
                body: JSON.stringify({
                    error: "result_rejected",
                    reason: "inhuman_rhythm",
                    message: "the keystroke rhythm is too regular to be human",
                }),
            }),
        );
        await openApp(page);
        const words = await chooseMode(page, "10");

        await page.locator("#tw-stage").click();
        await typeLikeHuman(page, words.join(" "));

        await expect(page.locator("[data-corner=readout]")).toHaveText(
            "Not scored: the keystroke rhythm is too regular to be human.",
        );
        await expect(page.locator("[data-corner=hint]")).toHaveText("Esc to try again");
    });

    test("an unreachable server is reported on load", async ({ page }, testInfo) => {
        test.skip(testInfo.project.name !== "desktop", "error copy does not depend on size");
        await page.route("**/api/tests", (route) => route.abort("connectionrefused"));

        await page.goto("/");

        await expect(page.locator("[data-corner=readout]")).toHaveText(
            "No sheet: the server could not be reached.",
            { timeout: 30_000 },
        );
    });
});
