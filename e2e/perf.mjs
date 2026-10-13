#!/usr/bin/env node
// Measures the frontend's main-thread cost in a real Chrome against a running stack: busy time
// per second while a time test runs with no keys, and busy time per key while typing. Start
// the stack with `node run.mjs --keep`, then pass its URL. See README.md.

import { parseArgs } from "node:util";

import { chromium } from "@playwright/test";

const { values: options } = parseArgs({
    options: {
        url: { type: "string" },
        runs: { type: "string", default: "5" },
        throttle: { type: "string", default: "1" },
    },
});
if (!options.url) {
    console.error("Usage: node perf.mjs --url <stack URL> [--runs 5] [--throttle 4]");
    process.exit(2);
}

const IDLE_MS = 5_000;
const KEYS = 200;
const KEY_GAP_MS = 80;

const PARTS = ["TaskDuration", "ScriptDuration", "RecalcStyleDuration", "LayoutDuration"];

/** Chrome's cumulative main-thread times, in milliseconds: all tasks, then their main parts. */
async function busyMs(cdp) {
    const { metrics } = await cdp.send("Performance.getMetrics");
    const value = (name) => metrics.find((metric) => metric.name === name).value * 1_000;
    return PARTS.map(value);
}

const minus = (after, before) => after.map((value, index) => value - before[index]);

async function openTest(browser, mode, amount) {
    const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
    const cdp = await page.context().newCDPSession(page);
    await cdp.send("Performance.enable");
    await cdp.send("Emulation.setCPUThrottlingRate", { rate: Number(options.throttle) });
    await page.goto(options.url);
    await page.locator("[data-corner=readout]").filter({ hasText: "ready" }).waitFor();
    const issued = page.waitForResponse((response) => response.url().endsWith("/api/tests"));
    const label = mode === "time" ? `${amount}s` : `${amount}`;
    await page.getByRole("button", { name: label, exact: true }).click();
    const { words } = await (await issued).json();
    await page.locator("[data-corner=readout]").filter({ hasText: "ready" }).waitFor();
    await page.locator("#tw-stage").focus();
    return { page, cdp, words };
}

/** Busy milliseconds per second while a 120 s time test runs and nobody types. */
async function idleRun(browser) {
    const { page, cdp } = await openTest(browser, "time", 120);
    await page.keyboard.type("a");
    await page.waitForTimeout(500);
    const before = await busyMs(cdp);
    await page.waitForTimeout(IDLE_MS);
    const busy = minus(await busyMs(cdp), before);
    const left = Number(await page.locator(".tw-count").textContent());
    if (!(left > 0 && left < 120)) throw new Error(`the time test is not counting down: ${left}`);
    await page.close();
    return busy.map((ms) => ms / (IDLE_MS / 1_000));
}

/** Busy milliseconds per key while typing a 100 word test at a steady pace. */
async function typingRun(browser) {
    const { page, cdp, words } = await openTest(browser, "words", 100);
    const keys = words.join(" ").slice(0, KEYS);
    const before = await busyMs(cdp);
    for (const key of keys) {
        await page.keyboard.type(key);
        await page.waitForTimeout(KEY_GAP_MS);
    }
    const busy = minus(await busyMs(cdp), before);
    const done = Number((await page.locator(".tw-count").textContent()).split("/")[0]);
    if (done < 20) throw new Error(`only ${done} words were typed`);
    await page.close();
    return busy.map((ms) => ms / KEYS);
}

/** The median, min and max of each part across runs. */
function summary(runs) {
    return PARTS.map((name, index) => {
        const sorted = runs.map((run) => run[index]).sort((a, b) => a - b);
        const median = sorted[Math.floor(sorted.length / 2)];
        const part = name.replace("Duration", "").padEnd(11);
        return `  ${part} median ${median.toFixed(2)}, min ${sorted[0].toFixed(2)}, max ${sorted.at(-1).toFixed(2)}`;
    }).join("\n");
}

const browser = await chromium.launch();
const idle = [];
const typing = [];
for (let run = 0; run < Number(options.runs); run += 1) {
    idle.push(await idleRun(browser));
    typing.push(await typingRun(browser));
}
await browser.close();
console.log(`CPU throttle ${options.throttle}x, ${options.runs} runs`);
console.log(`idle test running, busy ms per second:\n${summary(idle)}`);
console.log(`typing, busy ms per key (${KEYS} keys, ${KEY_GAP_MS} ms apart):\n${summary(typing)}`);
console.log(JSON.stringify({ throttle: Number(options.throttle), idle, typing }));
