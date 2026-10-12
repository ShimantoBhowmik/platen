#!/usr/bin/env node
// End-to-end run of the whole platform, the way a learner meets it: a throwaway Postgres, the
// API server built from this checkout, the frontend bundle it serves, and Playwright driving a
// real browser against them. Everything is torn down afterwards. Exit code 0 means every check
// passed. See README.md.

import { spawn, spawnSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, openSync, rmSync } from "node:fs";
import { createServer } from "node:net";
import { homedir, tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, "..");
const RESULTS = join(HERE, "results");
const WEB_RELEASE_OUT = join(HERE, ".build", "web");
const WEB_DEBUG_PUBLIC = join(ROOT, "web", "target", "dx", "type-cast-web", "debug", "web", "public");

const USAGE = `Usage: node run.mjs [options]

Runs the whole platform end to end: Postgres, the API server, the frontend and a browser.

Options:
  --skip-build        Reuse the last server and frontend builds
  --web <kind>        Frontend build: release (default, what users get) or debug (faster)
  --grep <pattern>    Run only the checks whose title matches
  --project <name>    Run only one project: api, desktop, laptop or phone
  --headed            Show the browser
  --keep              Leave the stack running after the checks, until Ctrl-C
  -h, --help          Show this help

Environment:
  E2E_DATABASE_URL    Use this database instead of a throwaway one. It must be disposable:
                      the checks write results and leaderboard rows.
  PG_BIN              Directory with initdb, pg_ctl and createdb, if they are not on PATH`;

const { values: options } = parseArgs({
    options: {
        "skip-build": { type: "boolean", default: false },
        web: { type: "string", default: "release" },
        grep: { type: "string" },
        project: { type: "string" },
        headed: { type: "boolean", default: false },
        keep: { type: "boolean", default: false },
        help: { type: "boolean", short: "h", default: false },
    },
});
if (options.help) {
    console.log(USAGE);
    process.exit(0);
}
if (!["release", "debug"].includes(options.web)) {
    console.error(`--web must be release or debug, not ${options.web}`);
    process.exit(2);
}

// rustup's toolchain first: it has the wasm target that a system rustc may lack.
const ENV = { ...process.env, PATH: `${join(homedir(), ".cargo", "bin")}:${process.env.PATH}` };
const cleanups = [];

function step(message) {
    console.log(`\n== ${message}`);
}

/** Runs a command to completion and throws with its output if it fails. */
function run(command, args, { cwd = ROOT, env = ENV, quiet = false } = {}) {
    const result = spawnSync(command, args, {
        cwd,
        env,
        stdio: quiet ? "pipe" : "inherit",
        encoding: "utf8",
    });
    if (result.error) {
        throw new Error(`${command} could not start: ${result.error.message}`);
    }
    if (result.status !== 0) {
        const output = quiet ? `\n${result.stdout}${result.stderr}` : "";
        throw new Error(`${command} ${args.join(" ")} exited with ${result.status}${output}`);
    }
    return result.stdout;
}

/** A free TCP port on the loopback interface. */
function freePort() {
    return new Promise((resolvePort, reject) => {
        const server = createServer();
        server.unref();
        server.on("error", reject);
        server.listen(0, "127.0.0.1", () => {
            const { port } = server.address();
            server.close(() => resolvePort(port));
        });
    });
}

/** The directory that holds initdb, pg_ctl and createdb. */
function postgresBin() {
    if (process.env.PG_BIN) {
        return process.env.PG_BIN;
    }
    const which = spawnSync("sh", ["-c", "command -v pg_ctl"], { encoding: "utf8" });
    if (which.status === 0 && which.stdout.trim()) {
        return dirname(which.stdout.trim());
    }
    for (const version of [17, 16, 15, 14]) {
        const candidate = `/opt/homebrew/opt/postgresql@${version}/bin`;
        if (existsSync(join(candidate, "pg_ctl"))) {
            return candidate;
        }
    }
    throw new Error("Postgres binaries not found. Install Postgres, set PG_BIN, or set E2E_DATABASE_URL.");
}

/** Starts a throwaway Postgres cluster in a temporary directory and returns its URL. */
async function startPostgres() {
    if (process.env.E2E_DATABASE_URL) {
        console.log("Using E2E_DATABASE_URL.");
        return process.env.E2E_DATABASE_URL;
    }
    const bin = postgresBin();
    const dir = mkdtempSync(join(tmpdir(), "type-cast-e2e-"));
    const data = join(dir, "data");
    const port = await freePort();
    // A fixed locale keeps initdb from failing on machines with an unusual LANG.
    const pgEnv = { ...ENV, LC_ALL: "C", LANG: "C" };
    run(join(bin, "initdb"), ["-D", data, "-U", "e2e", "--auth=trust", "-E", "UTF8"], {
        env: pgEnv,
        quiet: true,
    });
    run(
        join(bin, "pg_ctl"),
        [
            "-D", data,
            "-l", join(dir, "postgres.log"),
            "-o", `-p ${port} -k ${dir} -c listen_addresses=127.0.0.1`,
            "-w", "start",
        ],
        { env: pgEnv, quiet: true },
    );
    cleanups.push(() => {
        spawnSync(join(bin, "pg_ctl"), ["-D", data, "-m", "immediate", "stop"], { env: pgEnv });
        rmSync(dir, { recursive: true, force: true });
    });
    run(join(bin, "createdb"), ["-h", "127.0.0.1", "-p", String(port), "-U", "e2e", "typecast"], {
        env: pgEnv,
        quiet: true,
    });
    console.log(`Throwaway Postgres on port ${port}.`);
    return `postgres://e2e@127.0.0.1:${port}/typecast`;
}

/** Builds the server and the frontend; returns the directory the server serves. */
function build() {
    const staticDir = options.web === "release" ? join(WEB_RELEASE_OUT, "public") : WEB_DEBUG_PUBLIC;
    if (options["skip-build"]) {
        console.log("Skipping builds.");
    } else {
        run("cargo", ["build", "--locked"]);
        if (options.web === "release") {
            run("dx", ["bundle", "--platform", "web", "--release", "--out-dir", WEB_RELEASE_OUT], {
                cwd: join(ROOT, "web"),
            });
        } else {
            run("dx", ["build", "--platform", "web"], { cwd: join(ROOT, "web") });
        }
    }
    for (const required of [join(ROOT, "target", "debug", "type-cast"), join(staticDir, "index.html")]) {
        if (!existsSync(required)) {
            throw new Error(`Missing build output ${required}. Run without --skip-build.`);
        }
    }
    return staticDir;
}

/** Starts the API server and waits until it answers `/health`. Returns its base URL. */
async function startServer(databaseUrl, staticDir) {
    const port = await freePort();
    const log = openSync(join(RESULTS, "server.log"), "w");
    const server = spawn(join(ROOT, "target", "debug", "type-cast"), [], {
        cwd: ROOT,
        env: {
            ...ENV,
            DATABASE_URL: databaseUrl,
            // A fresh secret per run: tokens from any other run are worthless here.
            TOKEN_SECRET: randomBytes(32).toString("hex"),
            BIND_ADDR: `127.0.0.1:${port}`,
            STATIC_DIR: staticDir,
            RUST_LOG: "info",
        },
        stdio: ["ignore", log, log],
    });
    cleanups.push(() => server.kill("SIGTERM"));
    const baseUrl = `http://127.0.0.1:${port}`;
    const deadline = Date.now() + 30_000;
    while (Date.now() < deadline) {
        if (server.exitCode !== null) {
            throw new Error(`The server exited with ${server.exitCode}. See results/server.log.`);
        }
        try {
            const response = await fetch(`${baseUrl}/health`);
            if (response.ok) {
                console.log(`Server on ${baseUrl}.`);
                return baseUrl;
            }
        } catch {
            // Not listening yet.
        }
        await new Promise((wake) => setTimeout(wake, 200));
    }
    throw new Error("The server did not answer /health within 30 s. See results/server.log.");
}

/** Runs the Playwright checks against the stack and returns their exit code. */
function runChecks(baseUrl) {
    if (!existsSync(join(HERE, "node_modules", "@playwright", "test"))) {
        run("npm", ["ci", "--silent"], { cwd: HERE });
    }
    // A no-op when the pinned browser is already cached.
    run("npx", ["playwright", "install", "chromium"], { cwd: HERE, quiet: true });
    const args = ["playwright", "test"];
    if (options.grep) args.push("--grep", options.grep);
    if (options.project) args.push("--project", options.project);
    if (options.headed) args.push("--headed");
    const checks = spawnSync("npx", args, {
        cwd: HERE,
        env: { ...ENV, BASE_URL: baseUrl, E2E_WEB: options.web },
        stdio: "inherit",
    });
    return checks.status ?? 1;
}

function cleanUp() {
    while (cleanups.length > 0) {
        try {
            cleanups.pop()();
        } catch (error) {
            console.error(`Cleanup failed: ${error.message}`);
        }
    }
}

process.on("SIGINT", () => {
    cleanUp();
    process.exit(130);
});

let exitCode = 1;
try {
    rmSync(RESULTS, { recursive: true, force: true });
    mkdirSync(RESULTS, { recursive: true });
    step("Database");
    const databaseUrl = await startPostgres();
    step(`Build (server debug, frontend ${options.web})`);
    const staticDir = build();
    step("Server");
    const baseUrl = await startServer(databaseUrl, staticDir);
    step("Checks");
    exitCode = runChecks(baseUrl);
    if (options.keep) {
        console.log(`\nThe stack stays up at ${baseUrl}. Press Ctrl-C to stop it.`);
        await new Promise(() => {});
    }
} catch (error) {
    console.error(`\nE2E setup failed: ${error.message}`);
    exitCode = 2;
} finally {
    cleanUp();
}

step(exitCode === 0 ? "PASSED" : "FAILED");
console.log(`Screenshots: ${join(RESULTS, "screens")}`);
console.log(`Server log:  ${join(RESULTS, "server.log")}`);
if (exitCode !== 0) {
    console.log(`Failure traces: npx playwright show-trace ${join(RESULTS, "artifacts")}/<test>/trace.zip`);
}
process.exit(exitCode);
