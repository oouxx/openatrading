#!/usr/bin/env node
/**
 * Capture Jiuyangongshe browser-issued timestamp/token through headless Chrome CDP.
 * Requires Node >= 22 (built-in fetch/WebSocket) and a local Chrome/Chromium.
 */
import { spawn } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const HOME_URL = "https://www.jiuyangongshe.com/";
const API_MARKER = "web-api.jiuyangongshe.com";
const TIMEOUT_MS = 45_000;

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function findChrome() {
  const candidates = [
    process.env.JIUYAN_CHROME_PATH,
    process.platform === "darwin" &&
      "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    process.platform === "darwin" &&
      "/Applications/Chromium.app/Contents/MacOS/Chromium",
    process.platform === "darwin" &&
      "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    process.platform === "linux" && "/usr/bin/google-chrome",
    process.platform === "linux" && "/usr/bin/google-chrome-stable",
    process.platform === "linux" && "/usr/bin/chromium",
    process.platform === "linux" && "/usr/bin/chromium-browser",
    process.platform === "win32" &&
      join(
        process.env.PROGRAMFILES || "C:\\Program Files",
        "Google",
        "Chrome",
        "Application",
        "chrome.exe",
      ),
    process.platform === "win32" &&
      join(
        process.env["PROGRAMFILES(X86)"] || "C:\\Program Files (x86)",
        "Microsoft",
        "Edge",
        "Application",
        "msedge.exe",
      ),
  ].filter(Boolean);
  return candidates.find((candidate) => existsSync(candidate));
}

async function waitForDevToolsPort(userDataDir, chrome) {
  const portFile = join(userDataDir, "DevToolsActivePort");
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    if (chrome.exitCode !== null) {
      throw new Error(`Chrome exited before DevTools was ready (code ${chrome.exitCode})`);
    }
    if (existsSync(portFile)) {
      const [port] = readFileSync(portFile, "utf8").trim().split(/\r?\n/);
      if (port) return Number(port);
    }
    await sleep(100);
  }
  throw new Error("Timed out waiting for Chrome DevTools port");
}

class CdpConnection {
  constructor(url) {
    this.nextId = 1;
    this.pending = new Map();
    this.listeners = new Set();
    this.ws = new WebSocket(url);
    this.ready = new Promise((resolve, reject) => {
      this.ws.addEventListener("open", resolve, { once: true });
      this.ws.addEventListener("error", reject, { once: true });
    });
    this.ws.addEventListener("message", (event) => {
      const message = JSON.parse(String(event.data));
      if (message.id) {
        const pending = this.pending.get(message.id);
        if (!pending) return;
        this.pending.delete(message.id);
        if (message.error) pending.reject(new Error(message.error.message));
        else pending.resolve(message.result);
        return;
      }
      for (const listener of this.listeners) listener(message);
    });
  }

  async send(method, params = {}, sessionId) {
    await this.ready;
    const id = this.nextId++;
    const payload = { id, method, params };
    if (sessionId) payload.sessionId = sessionId;
    const result = new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
    });
    this.ws.send(JSON.stringify(payload));
    return result;
  }

  onEvent(listener) {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  close() {
    this.ws.close();
  }
}

async function captureSignature() {
  const chromePath = findChrome();
  if (!chromePath) {
    throw new Error(
      "Chrome/Chromium not found. Set JIUYAN_CHROME_PATH to a local browser executable.",
    );
  }

  const userDataDir = mkdtempSync(join(tmpdir(), "jiuyan-token-"));
  const args = [
    "--headless=new",
    "--remote-debugging-port=0",
    `--user-data-dir=${userDataDir}`,
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-background-networking",
    "--disable-component-update",
    "--disable-sync",
    "about:blank",
  ];
  if (process.env.JIUYAN_CHROME_NO_SANDBOX === "1") args.unshift("--no-sandbox");

  const chrome = spawn(chromePath, args, { stdio: "ignore" });
  let cdp;
  try {
    const port = await waitForDevToolsPort(userDataDir, chrome);
    const version = await fetch(`http://127.0.0.1:${port}/json/version`).then((r) => r.json());
    cdp = new CdpConnection(version.webSocketDebuggerUrl);
    const { targetId } = await cdp.send("Target.createTarget", { url: "about:blank" });
    const { sessionId } = await cdp.send("Target.attachToTarget", {
      targetId,
      flatten: true,
    });

    let resolveSignature;
    let rejectSignature;
    const signaturePromise = new Promise((resolve, reject) => {
      resolveSignature = resolve;
      rejectSignature = reject;
    });
    const timer = setTimeout(
      () => rejectSignature(new Error("Timed out capturing Jiuyan timestamp/token")),
      TIMEOUT_MS,
    );
    cdp.onEvent((message) => {
      if (
        message.sessionId !== sessionId ||
        message.method !== "Network.requestWillBeSent"
      ) {
        return;
      }
      const request = message.params?.request;
      if (!request?.url?.includes(API_MARKER)) return;
      const headers = request.headers || {};
      if (headers.timestamp && headers.token) {
        resolveSignature({
          timestamp: String(headers.timestamp),
          token: String(headers.token),
        });
      }
    });
    await cdp.send("Network.enable", {}, sessionId);
    await cdp.send("Page.enable", {}, sessionId);
    await cdp.send("Page.navigate", { url: HOME_URL }, sessionId);

    const signature = await signaturePromise.finally(() => clearTimeout(timer));
    await sleep(800);
    let cookies = [];
    try {
      const cookiesResult = await cdp.send("Storage.getCookies");
      cookies = cookiesResult.cookies || [];
    } catch {}
    const sessionCookie = cookies.find(
      (cookie) =>
        cookie.name === "SESSION" &&
        String(cookie.domain || "").includes("jiuyangongshe"),
    );

    return {
      timestamp: signature.timestamp,
      token: signature.token,
      session_token: sessionCookie?.value || null,
      captured_at: new Date().toISOString(),
    };
  } finally {
    try {
      cdp?.close();
    } catch {}
    chrome.kill("SIGTERM");
    await Promise.race([
      new Promise((resolve) => chrome.once("exit", resolve)),
      sleep(2_000),
    ]);
    if (chrome.exitCode === null) chrome.kill("SIGKILL");
    rmSync(userDataDir, { recursive: true, force: true });
  }
}

captureSignature()
  .then((result) => {
    process.stdout.write(`${JSON.stringify(result)}\n`);
  })
  .catch((error) => {
    process.stderr.write(`get_token failed: ${error.message}\n`);
    process.exitCode = 1;
  });
