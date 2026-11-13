import { expect, test, type Page } from "@playwright/test";
import { choose, open, say, startSession, type SidekickTest } from "../ui/helpers";

/*
 * README screenshots: `pnpm docs:screenshots` rewrites docs/screenshots/app/*.png.
 * The real frontend runs against the mocked backend (as in the UI tests) with
 * canned model replies, so the pictures show realistic answers rather than the
 * mock's echo. Nothing here is asserted beyond what a picture needs.
 */

const OUT = "docs/screenshots/app";
const OVERLAY = { width: 420, height: 640 };
const SETTINGS = { width: 720, height: 900 };

/** Canned model replies: the first whose `match` is in the prompt is used. */
const REPLIES = [
  {
    match: "two numbers that add up",
    long: false,
    reply: "One pass with a Map of value to index: O(n) time, O(n) space.",
  },
  {
    match: "two numbers that add up",
    long: true,
    reply:
      "**Approach 1: brute force (most expensive).** Try every pair.\n\n```js\nfunction twoSumBrute(nums, target) {\n  for (let i = 0; i < nums.length; i++)\n    for (let j = i + 1; j < nums.length; j++)\n      if (nums[i] + nums[j] === target) return [i, j];\n  return [];\n}\n```\n\nTime **O(n²)**: every pair is checked. Space **O(1)**. At n = 10⁵ that is about 5 × 10⁹ checks: too slow.\n\n**Approach 2: optimal (least expensive).** Remember each value's index; for each number, look up its complement.\n\n```js\nfunction twoSum(nums, target) {\n  const seen = new Map();\n  for (let i = 0; i < nums.length; i++) {\n    const j = seen.get(target - nums[i]);\n    if (j !== undefined) return [j, i];\n    seen.set(nums[i], i);\n  }\n  return [];\n}\n```\n\nTime **O(n)**: one pass with O(1) lookups. Space **O(n)** for the map.",
  },
  {
    match: "running notes for a live meeting",
    reply:
      "- The partner moved every team to the **Pro plan** last week.\n- Pro stays at 600 requests a minute.\n- Billing data migration: plan due next.\n- Rollout timeline wanted by Friday.",
  },
  { match: "rate limit", long: false, reply: "600 requests a minute on Pro, bursts to 1,000." },
  {
    match: "rate limit",
    long: true,
    reply:
      "The **Pro tier allows 600 requests per minute** per API key, with short bursts up to 1,000 [1].\n\n- Limits are counted per key, not per account [1].\n- Going over returns `429` with a `Retry-After` header [1].\n- Enterprise plans can raise the limit on request.",
  },
  {
    match: "error mean",
    reply:
      "The screenshot shows a **`TypeError: Cannot read properties of undefined (reading 'map')`** in `InvoiceList.tsx` [1].\n\n`invoices` is still `undefined` on the first render, before the request returns. Default it to an empty list:\n\n```tsx\nconst { data: invoices = [] } = useInvoices();\n```",
  },
  {
    match: "What did I miss",
    reply:
      "- Priya confirmed the **Pro tier** stays at 600 requests a minute.\n- Sam asked for a migration plan for the billing data.\n- You agreed to send the rollout timeline by Friday.",
  },
  {
    match: "recap",
    reply:
      "- Priya confirmed the **Pro tier** stays at 600 requests a minute.\n- Sam asked for a migration plan for the billing data.\n- You agreed to send the rollout timeline by Friday.",
  },
];

async function setup(page: Page, which: "overlay" | "settings" = "overlay") {
  await page.setViewportSize(which === "settings" ? SETTINGS : OVERLAY);
  await page.addInitScript((replies) => {
    (window as unknown as { __sidekickMockOptions: unknown }).__sidekickMockOptions = {
      replies,
      version: "0.1.0",
      protection: {
        state: "protected",
        reason: "Hidden from screen sharing and screenshots by other apps.",
      },
    };
  }, REPLIES);
  await open(page, which);
  await hook(page, (t) => t.commands.updateSettings({ profile: { name: "Alex", nicknames: [] } }));
}

type Hook = SidekickTest & {
  commands: SidekickTest["commands"] & { recordLatency(stage: string, ms: number): Promise<void> };
};

/** Runs `fn` with the test hook in the page. */
function hook(page: Page, fn: (t: Hook) => unknown) {
  return page.evaluate(
    `(async () => { const t = window.__sidekickTest; return (${fn.toString()})(t); })()`,
  );
}

/** A line from someone in the call, past the question-detection debounce. */
async function line(page: Page, text: string, speaker = "speaker_1") {
  await say(page, text, speaker);
  await page.waitForTimeout(900);
}

const shot = (page: Page, name: string) =>
  page.screenshot({ path: `${OUT}/${name}.png`, animations: "disabled" });

test("live answers", async ({ page }) => {
  await setup(page);
  await startSession(page, "Partner API review");
  await line(page, "Thanks for joining. We moved everyone to the Pro plan last week.");
  await line(page, "Alex, what's the rate limit on the Pro tier?");
  await page.getByRole("tab", { name: "Live Answers" }).click();
  await expect(page.getByRole("article").first()).toContainText("600 requests per minute");
  await shot(page, "live-answers");
});

test("transcript", async ({ page }) => {
  await setup(page);
  await startSession(page, "Partner API review");
  await line(page, "Thanks for joining. We moved everyone to the Pro plan last week.");
  await line(page, "Great, and the new dashboard is live for every team.", "me");
  await line(page, "How should we migrate the old billing data?", "speaker_2");
  await line(page, "Let's plan that next. Can we have a timeline by Friday?");
  await page.getByRole("tab", { name: "Transcript" }).click();
  await shot(page, "transcript");
});

test("chat about a screenshot", async ({ page }) => {
  await setup(page);
  await startSession(page, "Bug triage");
  await page.getByRole("tab", { name: "Chat" }).click();
  await page.getByRole("button", { name: "Screenshot", exact: true }).click();
  await page.getByRole("menuitem", { name: "Full screen" }).click();
  const box = page.getByRole("textbox", { name: "Ask a question" });
  await box.fill("What does this error mean?");
  await box.press("Enter");
  await expect(page.getByRole("article").first()).toContainText("TypeError");
  await shot(page, "chat-screenshot");
});

test("DS & Algo answer", async ({ page }) => {
  await setup(page);
  await startSession(page, "Coding round");
  await choose(page.getByRole("combobox", { name: "Skill" }), "dsa-interview");
  await line(
    page,
    "Alex, given an array of integers and a target, return the indices of the two numbers that add up to the target.",
  );
  await page.getByRole("tab", { name: "Live Answers" }).click();
  await expect(page.getByRole("article").first()).toContainText("Approach 2");
  await page.setViewportSize({ width: 420, height: 1100 });
  await shot(page, "dsa-answer");
});

test("screenshot quick actions", async ({ page }) => {
  await setup(page);
  await startSession(page, "Bug triage");
  await page.getByRole("tab", { name: "Chat" }).click();
  await page.getByRole("button", { name: "Screenshot", exact: true }).click();
  await page.getByRole("menuitem", { name: "Full screen" }).click();
  await expect(page.getByRole("group", { name: "Quick actions" })).toBeVisible();
  await shot(page, "screenshot-actions");
});

test("notes: agenda and what did I miss", async ({ page }) => {
  await setup(page);
  await startSession(page, "Partner API review");
  await page.getByRole("tab", { name: "Notes" }).click();
  const agenda = page.getByRole("region", { name: "Agenda" });
  await agenda.getByRole("button", { name: "Add agenda" }).click();
  await agenda
    .getByLabel("Agenda items, one per line")
    .fill("Pro tier limits\nBilling data migration\nRollout timeline");
  await agenda.getByRole("button", { name: "Save agenda" }).click();
  await choose(agenda.getByRole("combobox", { name: "Status of Pro tier limits" }), "covered");
  await choose(
    agenda.getByRole("combobox", { name: "Status of Billing data migration" }),
    "in_progress",
  );
  await line(page, "Priya here: the Pro tier stays at 600 requests a minute.");
  await page.getByRole("button", { name: "What did I miss?" }).click();
  await expect(page.getByRole("region", { name: "What you missed" })).toContainText("Friday");
  await shot(page, "notes");
});

test("summary and action items", async ({ page }) => {
  await setup(page);
  await startSession(page, "Partner API review");
  await line(page, "Priya will send the quote by Friday.");
  await page.getByRole("button", { name: "End", exact: true }).click();
  await page.getByRole("tab", { name: "Notes" }).click();
  await expect(page.getByRole("region", { name: "Summary" })).toBeVisible();
  await page.getByRole("tab", { name: "Transcript" }).click();
  await hook(page, async (t) => {
    const r = await t.commands.listSessions(null, null);
    const id = r.status === "ok" ? r.data.items[0]?.id : undefined;
    if (!id) throw new Error("no session");
    await t.commands.saveSummary(
      id,
      "The partner stays on the **Pro tier** (600 requests a minute). Billing data moves to the new store in three steps: dual-write, backfill, then switch reads.\n\n## Decisions\n\n- Keep the current rate limits for the launch.\n- Migrate billing data before the rollout.",
      [
        { text: "Send the rollout timeline", owner: "Alex", due: "Friday" },
        { text: "Draft the migration plan", owner: "Sam", due: "Next Tuesday" },
        { text: "Share the Pro tier quote", owner: "Priya", due: null },
      ],
    );
  });
  await page.getByRole("tab", { name: "Notes" }).click();
  await page.getByRole("checkbox", { name: /Share the Pro tier quote/ }).check();
  await shot(page, "summary");
});

test("consent before listening", async ({ page }) => {
  await setup(page);
  await page.getByLabel("Session name").fill("Partner API review");
  await page.getByRole("button", { name: "New session" }).click();
  await page.getByRole("button", { name: "Start", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Before Sidekick listens" });
  await dialog.getByLabel("The other people and me").check();
  await shot(page, "consent");
});

test("documents and knowledge packs", async ({ page }) => {
  await setup(page);
  await hook(page, async (t) => {
    const p = await t.commands.createPack("Sales kit");
    await t.commands.ingestPackDocument(p.data.id, "/docs/price-list.md");
  });
  await startSession(page, "Partner API review");
  await hook(page, (t) => {
    t.files.picked = ["/docs/API pricing 2026.pdf", "/docs/Rate limits.docx", "/docs/Roadmap.pptx"];
  });
  await page.getByRole("button", { name: /Documents \(/ }).click();
  await page.getByRole("button", { name: "Add files" }).click();
  await expect(page.getByRole("button", { name: /Documents \(3\/3 ready\)/ })).toBeVisible();
  await page.getByRole("checkbox", { name: /Sales kit/ }).check();
  await shot(page, "documents");
});

test("skills", async ({ page }) => {
  await setup(page);
  await startSession(page, "System design interview");
  await choose(page.getByRole("combobox", { name: "Skill" }), "system-design-backend");
  await page.getByRole("button", { name: /More Skills/ }).click();
  await page
    .getByRole("dialog", { name: "Also use" })
    .getByRole("checkbox", { name: "System Design Round - Frontend (JS)" })
    .check();
  await shot(page, "skills-picker");
});

test("light theme", async ({ page }) => {
  await setup(page);
  await hook(page, (t) => t.commands.updateSettings({ theme: "light" }));
  await startSession(page, "Partner API review");
  await line(page, "Alex, what's the rate limit on the Pro tier?");
  await page.getByRole("tab", { name: "Live Answers" }).click();
  await expect(page.getByRole("article").first()).toContainText("600 requests per minute");
  await shot(page, "theme-light");
});

test("glass over a call", async ({ page }) => {
  await setup(page);
  // A bright, busy stand-in for the call behind the transparent window.
  await page.addStyleTag({
    content: `html { background:
      radial-gradient(circle at 30% 30%, #f2c9a0 0 14%, transparent 15%),
      radial-gradient(circle at 72% 62%, #9fd3f5 0 16%, transparent 17%),
      linear-gradient(135deg, #3b6fd6 0 33%, #e9edf3 33% 66%, #2f9e6b 66%) !important; }`,
  });
  await startSession(page, "Partner API review");
  await line(page, "Alex, what's the rate limit on the Pro tier?");
  await page.getByRole("tab", { name: "Live Answers" }).click();
  await expect(page.getByRole("article").first()).toContainText("600 requests per minute");
  await page.getByRole("button", { name: "View" }).click();
  await page.getByRole("slider", { name: "Background opacity" }).fill("0.6");
  await page.keyboard.press("Escape");
  await shot(page, "glass");
});

test("settings: providers", async ({ page }) => {
  await setup(page, "settings");
  const llm = page.getByRole("region", { name: /answers/i });
  await llm.getByLabel("Anthropic (Claude) API key").fill("sk-ant-demo-key-1234");
  await llm.getByRole("button", { name: "Save and test" }).click();
  await expect(llm.getByText(/Key works and is saved/)).toBeVisible();
  await page.getByLabel("Your name", { exact: true }).fill("Alex Kim");
  await page.getByLabel("New nickname").fill("AK");
  await page.getByRole("button", { name: "Add", exact: true }).click();
  await page.getByRole("heading", { level: 1 }).first().scrollIntoViewIfNeeded();
  await shot(page, "settings");
});

test("settings: skills", async ({ page }) => {
  await setup(page, "settings");
  const skills = page.getByRole("region", { name: /^Skills/ });
  await skills.scrollIntoViewIfNeeded();
  await skills.screenshot({ path: `${OUT}/settings-skills.png`, animations: "disabled" });
});

test("settings: diagnostics", async ({ page }) => {
  await setup(page, "settings");
  await hook(page, async (t) => {
    const stages: [string, number[]][] = [
      ["transcript_lag", [620, 710, 540, 880, 760]],
      ["question_to_cue", [900, 1100, 950, 1250]],
      ["question_to_answer", [2400, 2900, 3100, 2650]],
      ["llm_first_token", [420, 510, 380, 600]],
      ["retrieval", [18, 25, 12, 30]],
    ];
    for (const [stage, values] of stages)
      for (const ms of values) await t.commands.recordLatency(stage, ms);
  });
  const panel = page.getByRole("region", { name: "Diagnostics" });
  await panel.getByRole("button", { name: "Refresh" }).click();
  await panel.scrollIntoViewIfNeeded();
  await panel.screenshot({ path: `${OUT}/diagnostics.png`, animations: "disabled" });
});
