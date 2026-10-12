// Helpers shared by the end-to-end checks.

import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { test as base, expect } from "@playwright/test";

const SCREENS = join(dirname(fileURLToPath(import.meta.url)), "..", "results", "screens");

/**
 * Deterministic key gaps of 60 to 160 ms. The server rejects a rhythm that is too even to be
 * human, so a fixed delay would fail.
 */
export function humanGaps(count, seed = 7) {
    let state = seed;
    return Array.from({ length: count }, () => {
        state = (state * 1103515245 + 12345) % 2147483648;
        return 60 + Math.floor((state / 2147483648) * 100);
    });
}

/** Milliseconds since the first key for each gap list, as the API expects. */
export function timesFrom(gaps) {
    let now = 0;
    return gaps.map((gap, index) => (index === 0 ? 0 : (now += gap)));
}

/** A letter that differs from `letter`, for typing a deliberate mistake. */
export function wrongLetter(letter) {
    return letter === "z" ? "q" : "z";
}

/**
 * The keys for typing `words` with two deliberate mistakes: word 2 keeps a wrong first letter,
 * and word 4 gets a wrong letter that is corrected with backspace.
 */
export function keysWithMistakes(words) {
    return words
        .map((word, index) => {
            if (index === 2) return wrongLetter(word[0]) + word.slice(1);
            if (index === 4) return `${wrongLetter(word[0])}\b${word}`;
            return word;
        })
        .join(" ");
}

/** Types `keys` on the page's keyboard with human gaps; `\b` is backspace. */
export async function typeLikeHuman(page, keys, seed = 7) {
    const gaps = humanGaps(keys.length, seed);
    for (const [index, key] of [...keys].entries()) {
        if (key === "\b") {
            await page.keyboard.press("Backspace");
        } else {
            await page.keyboard.type(key);
        }
        await page.waitForTimeout(gaps[index]);
    }
}

/**
 * Saves a full-page screenshot under results/screens for visual review, once every running
 * animation (such as the chart's trace) has settled, so the shot shows the resting state.
 */
export async function screen(page, testInfo, name) {
    mkdirSync(SCREENS, { recursive: true });
    await page.waitForFunction(() =>
        document.getAnimations().every((animation) => animation.playState !== "running"),
    );
    await page.screenshot({ path: join(SCREENS, `${testInfo.project.name}-${name}.png`), fullPage: true });
}

/** Page metrics for overflow checks. */
export function pageSize(page) {
    return page.evaluate(() => ({
        scrollWidth: document.documentElement.scrollWidth,
        scrollHeight: document.documentElement.scrollHeight,
        width: innerWidth,
        height: innerHeight,
        scrollY,
    }));
}

/**
 * The test fixture, extended so every page records console errors and uncaught exceptions,
 * and each check fails if any appeared. A check that provokes an error on purpose lists the
 * expected messages with `test.use({ allowedConsoleErrors: [/pattern/] })`.
 */
export const test = base.extend({
    allowedConsoleErrors: [[], { option: true }],
    page: async ({ page, allowedConsoleErrors }, use) => {
        const errors = [];
        // The debug build tries the dev server's hot-reload socket, which does not exist here.
        const allowed = [/\/_dioxus/, ...allowedConsoleErrors];
        page.on("pageerror", (error) => errors.push(`exception: ${error.message}`));
        page.on("console", (message) => {
            const text = message.text();
            if (message.type() === "error" && !allowed.some((pattern) => pattern.test(text))) {
                errors.push(`console: ${text}`);
            }
        });
        await use(page);
        expect(errors, "console errors and uncaught exceptions").toEqual([]);
    },
});

export { expect };
