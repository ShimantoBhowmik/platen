// The HTTP API, checked through the running server and a real database.

import { expect, humanGaps, keysWithMistakes, test, timesFrom } from "./support.mjs";

const MODES = [
    ["time", 15], ["time", 30], ["time", 60], ["time", 120],
    ["words", 10], ["words", 25], ["words", 50], ["words", 100],
];

/** Starts a test and notes when, so a submission can wait out its typing time. */
async function startTest(request, mode, amount) {
    const response = await request.post("/api/tests", { data: { mode, amount } });
    expect(response.status()).toBe(200);
    return { ...(await response.json()), startedAt: Date.now() };
}

/**
 * Submits keys with human timing. The server rejects a result that arrives before its typing
 * could have finished, so this waits until the claimed duration has passed.
 */
async function submit(request, issued, keys, { name, gaps = humanGaps(keys.length) } = {}) {
    const times = timesFrom(gaps);
    const typingMs = times[times.length - 1];
    const waitMs = issued.startedAt + typingMs + 300 - Date.now();
    if (waitMs > 0) await new Promise((wake) => setTimeout(wake, waitMs));
    return request.post("/api/results", { data: { token: issued.token, keys, times, name } });
}

test("health answers", async ({ request }) => {
    expect((await request.get("/health")).status()).toBe(200);
});

test("every mode issues a test with common lowercase words", async ({ request }) => {
    for (const [mode, amount] of MODES) {
        const issued = await startTest(request, mode, amount);
        expect(issued.token.length).toBeGreaterThan(20);
        expect(issued.mode).toEqual({ mode, amount });
        if (mode === "words") expect(issued.words).toHaveLength(amount);
        else expect(issued.words.length).toBeGreaterThan(amount * 5);
        for (const word of issued.words) expect(word).toMatch(/^[a-z]+$/);
    }
});

test("an unknown mode is refused", async ({ request }) => {
    const response = await request.post("/api/tests", { data: { mode: "time", amount: 45 } });
    expect(response.status()).toBe(400);
    expect((await response.json()).error).toBe("invalid_mode");
});

test("a typed word test is scored, stored, ranked and single-use", async ({ request }) => {
    const issued = await startTest(request, "words", 10);
    const name = `e2e-${Date.now() % 100000}`;
    const keys = keysWithMistakes(issued.words);

    const response = await submit(request, issued, keys, { name });

    expect(response.status()).toBe(201);
    const result = await response.json();
    const { stats, breakdown, timeline } = result;
    expect(result.name).toBe(name);
    expect(stats.wpm).toBeGreaterThan(0);
    expect(stats.raw_wpm).toBeGreaterThanOrEqual(stats.wpm);
    expect(stats.accuracy).toBeLessThan(100);
    // Word 2 kept a wrong letter, and the space that commits it wrong is a miss too. Word 4
    // had a corrected one.
    expect(stats.incorrect_keystrokes).toBe(3);
    expect(breakdown.missed_words).toEqual([
        { expected: issued.words[2], typed: keys.split(" ")[2] },
    ]);
    expect(breakdown.keys.filter((key) => key.misses > 0).length).toBeGreaterThan(0);
    // One point per scored second: a word test also scores a final half second or more.
    const points = Math.floor(stats.elapsed_ms / 1000) + (stats.elapsed_ms % 1000 >= 500 ? 1 : 0);
    expect(timeline).toHaveLength(points);
    for (const point of timeline) {
        expect(point.wpm).toBeGreaterThanOrEqual(0);
        expect(Number.isInteger(point.raw_wpm)).toBe(true);
    }
    expect(timeline.reduce((sum, point) => sum + point.errors, 0)).toBeLessThanOrEqual(3);

    const fetched = await request.get(`/api/results/${result.id}`);
    expect(fetched.status()).toBe(200);
    expect(await fetched.json()).toEqual(result);

    const board = await request.get("/api/leaderboard?mode=words&amount=10&limit=100");
    expect(board.status()).toBe(200);
    const entry = (await board.json()).entries.find((row) => row.name === name);
    expect(entry).toMatchObject({ result_id: result.id, wpm: stats.wpm });

    const again = await submit(request, issued, keys, { name });
    expect(again.status()).toBe(409);
    expect((await again.json()).error).toBe("already_submitted");
});

test("a result sent before it could have been typed is rejected", async ({ request }) => {
    const issued = await startTest(request, "words", 10);
    const keys = issued.words.join(" ");
    const times = timesFrom(humanGaps(keys.length));

    const response = await request.post("/api/results", {
        data: { token: issued.token, keys, times },
    });

    expect(response.status()).toBe(422);
    expect(await response.json()).toMatchObject({
        error: "result_rejected",
        reason: "submitted_too_early",
    });
});

test("a machine-even rhythm is rejected", async ({ request }) => {
    const issued = await startTest(request, "words", 10);
    const keys = issued.words.join(" ");

    const response = await submit(request, issued, keys, { gaps: keys.split("").map(() => 120) });

    expect(response.status()).toBe(422);
    expect((await response.json()).reason).toBe("inhuman_rhythm");
});

test("a tampered token and a bad name are refused", async ({ request }) => {
    const issued = await startTest(request, "words", 10);
    const keys = issued.words.join(" ");
    const times = timesFrom(humanGaps(keys.length));
    const flipped = issued.token.slice(0, -2) + (issued.token.endsWith("A") ? "BB" : "AA");

    const tampered = await request.post("/api/results", { data: { token: flipped, keys, times } });
    expect(tampered.status()).toBe(400);
    expect((await tampered.json()).error).toBe("invalid_token");

    const badName = await request.post("/api/results", {
        data: { token: issued.token, keys, times, name: "‮" },
    });
    expect(badName.status()).toBe(400);
    expect((await badName.json()).error).toBe("invalid_name");
});

test("an unknown result is not found", async ({ request }) => {
    const response = await request.get("/api/results/00000000-0000-4000-8000-000000000000");
    expect(response.status()).toBe(404);
    expect((await response.json()).error).toBe("not_found");
});
