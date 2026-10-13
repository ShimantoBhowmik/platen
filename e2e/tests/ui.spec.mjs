// The app in a real browser: typing with the keyboard, the countdown, the server-verified
// result plate, preferences and error states. Runs at desktop, laptop and phone sizes.

import { readFileSync } from "node:fs";

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

/** The next test the server issues. */
function nextTest(page) {
    return page.waitForResponse(
        (response) => response.url().endsWith("/api/tests") && response.request().method() === "POST",
    );
}

/**
 * Picks a test kind (`"time"` or `"words"`) and length on the bar and returns the words the
 * server issued for it.
 */
async function chooseMode(page, kind, label) {
    const kindButton = page.getByRole("group", { name: "Test kind" }).getByRole("button", { name: kind });
    if ((await kindButton.getAttribute("aria-pressed")) !== "true") {
        const switched = nextTest(page);
        await kindButton.click();
        await switched;
        await expect(page.locator("[data-corner=readout]")).toHaveText("ready");
    }
    const issued = nextTest(page);
    await page.getByRole("button", { name: label, exact: true }).click();
    const { words } = await (await issued).json();
    await expect(page.locator("[data-corner=readout]")).toHaveText("ready");
    return words;
}

/** The sheet's text layout, read from the figure the app embeds. */
const TEXT = JSON.parse(
    readFileSync(new URL("../../web/assets/typewriter.json", import.meta.url), "utf8"),
).text;
/** Cells a line of copy wraps at. */
const WRAP = TEXT.wrap;

/**
 * The sheet's lines: as many words as fit in `WRAP` cells, the same rule the app lays the
 * copy out with. The app justifies every line but the last, which only moves its words.
 */
function sheetLines(words) {
    const lines = [[]];
    for (const word of words) {
        const line = lines[lines.length - 1];
        if (line.length > 0 && [...line, word].join(" ").length > WRAP) lines.push([word]);
        else line.push(word);
    }
    return lines.map((line) => line.join(" "));
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
    const words = await chooseMode(page, "words", "10");
    const count = page.locator(".tw-count");
    await expect(count).toHaveText("0/10");
    await expect(page.locator("[data-corner=fig]")).toHaveCount(0);
    await expect(page.getByText(/^Current line: /)).toHaveText(`Current line: ${sheetLines(words)[0]}`);
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
    await expect(page.locator("[data-corner=readout]")).toHaveText("");
    const [wpmShown, accuracyShown] = await plate.locator(".tw-figures dd").allTextContents();
    expect(Math.abs(Number.parseInt(wpmShown, 10) - stats.wpm)).toBeLessThanOrEqual(0.5);
    expect(Math.abs(Number.parseInt(accuracyShown, 10) - stats.accuracy)).toBeLessThanOrEqual(0.5);
    await expect(stat(plate, "keys")).toHaveText(
        `${stats.correct_keystrokes}/${stats.incorrect_keystrokes}`,
    );
    await expect(stat(plate, "test")).toHaveText("words 10");
    await expect(plate.locator(".tw-stats .tw-sub").first()).toHaveText("english 200");
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
        // A bar button must not jump the page, even when the stage is off screen.
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
    await expect(page.locator(".tw-result")).toHaveCount(0);
    await expect(count).toHaveText("0/10");

    // So does Tab then Enter, also in the middle of a test.
    await page.keyboard.press("x");
    await expect(page.locator("[data-corner=readout]")).toHaveText("typing");
    await page.keyboard.press("Tab");
    await expect(page.getByRole("button", { name: "Tab + Enter restarts" })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.locator("[data-corner=readout]")).toHaveText("ready");
    await expect(count).toHaveText("0/10");
    await expect(page.locator("#tw-stage")).toBeFocused();
});

test("a time test counts down and is scored at its limit", async ({ page }, testInfo) => {
    test.skip(testInfo.project.name !== "desktop", "one size is enough for a 15 s wait");
    await openApp(page);
    const words = await chooseMode(page, "time", "15s");
    const count = page.locator(".tw-count");
    await expect(count).toHaveText("15");
    // The sheet shows the copy three lines at a time, each drawn letter by letter.
    const lines = sheetLines(words);
    for (const row of [0, 1, 2]) {
        await expect(page.locator(".tw-line").nth(row)).toHaveText(lines[row].replaceAll(" ", ""));
    }
    // Justified: every line's last letter sits in the last wrap cell.
    const flush = (TEXT.x0 + (WRAP - 0.5) * TEXT.cell).toFixed(2);
    for (const row of [0, 1, 2]) {
        await expect(page.locator(".tw-line").nth(row).locator("text").last()).toHaveAttribute("x", flush);
    }

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

test("a Caps Lock notice shows while Caps Lock is on and the card has focus", async ({ page }, testInfo) => {
    await openApp(page);
    const stage = page.locator("#tw-stage");
    const notice = page.getByRole("status").getByText("Caps Lock", { exact: true });
    // keyboard.press("CapsLock") leaves getModifierState false in headless Chromium, so keys carry it.
    const key = (type, key, code, capsLock) =>
        stage.dispatchEvent(type, { key, code, modifierCapsLock: capsLock, bubbles: true });
    await stage.click();
    await expect(notice).toHaveCount(0);

    await key("keydown", "CapsLock", "CapsLock", true);
    await expect(notice).toBeVisible();
    await screen(page, testInfo, "6-caps-lock");
    await key("keydown", "A", "KeyA", true);
    await key("keyup", "A", "KeyA", true);
    await expect(page.locator("[data-corner=readout]")).toHaveText("typing");
    await expect(notice).toBeVisible();
    await key("keyup", "CapsLock", "CapsLock", false);
    await expect(notice).toHaveCount(0);

    await key("keydown", "CapsLock", "CapsLock", true);
    await expect(notice).toBeVisible();
    await page.evaluate(() => document.activeElement.blur());
    await expect(notice).toBeHidden();
});

test("each test kind keeps the length last chosen for it", async ({ page }, testInfo) => {
    test.skip(testInfo.project.name !== "desktop", "the bar's logic does not depend on size");
    await openApp(page);
    const seconds = page.getByRole("group", { name: "Seconds" });
    await expect(seconds.getByRole("button", { name: "30s" })).toHaveAttribute("aria-pressed", "true");
    await chooseMode(page, "time", "60s");
    await chooseMode(page, "words", "50");
    await expect(page.locator(".tw-count")).toHaveText("0/50");

    await page.getByRole("button", { name: "time", exact: true }).click();
    await expect(seconds.getByRole("button", { name: "60s" })).toHaveAttribute("aria-pressed", "true");
    await expect(page.locator(".tw-count")).toHaveText("60");
    await page.getByRole("button", { name: "words", exact: true }).click();
    await expect(page.getByRole("group", { name: "Words" }).getByRole("button", { name: "50" })).toHaveAttribute(
        "aria-pressed",
        "true",
    );
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
    const words = await chooseMode(page, "words", "10");

    const keys = `${words[0]} ${words[1][0]}`;
    await typeLikeHuman(page, keys);
    await page.keyboard.press("Backspace");

    await expect.poll(() => page.evaluate(() => window.played.length)).toBe(keys.length + 1);
    const played = await page.evaluate(() => window.played);
    const backspace = played.pop();
    const [space] = played.splice(words[0].length, 1);
    // Clacks are 160 ms cuts of the recording; the space bar is a 130 ms darker clack, and the
    // backspace a lighter 120 ms tick.
    for (const clip of played) expect(clip.seconds).toBeCloseTo(0.16, 3);
    expect(space.seconds).toBeCloseTo(0.13, 3);
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
        const words = await chooseMode(page, "words", "10");

        await page.locator("#tw-stage").click();
        await typeLikeHuman(page, words.join(" "));

        await expect(page.locator("[data-corner=readout]")).toHaveText(
            "Not scored: the keystroke rhythm is too regular to be human.",
        );
        await expect(page.locator("[data-corner=hint]")).toHaveText("Tab + Enter to try again");
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
