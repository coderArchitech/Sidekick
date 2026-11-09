/**
 * Question detection (PRD QA-1, QA-2). Rules, not a model: fast (well inside
 * the 150 ms budget in tech doc 13.1), free, and measurable against
 * `questions.fixture.ts`. A small-model classifier can sit behind the same
 * interface later if real meetings show the need.
 */

const INTERROGATIVES = new Set([
  "what",
  "whats",
  "what's",
  "how",
  "hows",
  "how's",
  "when",
  "where",
  "who",
  "whos",
  "who's",
  "whom",
  "whose",
  "which",
  "why",
  "can",
  "could",
  "would",
  "will",
  "do",
  "does",
  "did",
  "is",
  "are",
  "was",
  "were",
  "should",
  "shall",
  "have",
  "has",
  "may",
  "might",
  "any",
  "mind",
]);

/** Openers that ask for something without being a question. */
const IMPLICIT = [
  /^(please )?(walk|talk|take|run) (us|me) through\b/,
  /^(please )?(tell|remind|show) (us|me)\b/,
  /^(please )?(explain|describe|summari[sz]e|clarify)\b/,
  // "Give me the numbers", but not "give me a second".
  /^(please )?give (us|me) (?!a (sec|second|minute|moment)\b|one (sec|second|minute|moment)\b)/,
  /^help (us|me) understand\b/,
  /^share your (thoughts|view|take)\b/,
  /^(your )?thoughts on\b/,
  /^i('d| would) (love|like) to (hear|know|understand)\b/,
  /^(i('m| am) )?curious (what|how|whether|if|about|why|where|when|who|which)\b/,
  /^i (was )?wonder(ing)? (if|whether|how|what|why)\b/,
  /^let (us|me) know (how|what|when|where|which|whether)\b/,
  /^fill (us|me) in\b/,
  /^can you talk about\b/,
  /^(quick )?update on\b/,
  /^over to you\b/,
  /^how about\b/,
];

/**
 * Tasks: interviewers and colleagues often ask for work, not information
 * ("Design a URL shortener", "Implement debounce", "Given an array…").
 * Checked after leading fillers ("okay so", "now") are removed.
 */
const TASK_VERBS =
  "design|implement|write|build|code|create|develop|architect|model|sketch|draw|solve|optimi[sz]e|refactor|debug|reverse|sort|find|merge|validate|parse|flatten|traverse|compute|calculate|convert|detect|count|make|extend|modify|improve|handle";
const TASKS = [
  // "Reverse a linked list", "Find the longest…", "Now make it work…"
  new RegExp(
    `^(please )?(${TASK_VERBS}) (a|an|the|me|us|this|that|it|two|three|your|some|all|each|every|our|one|n)\\b`,
  ),
  // Design-type verbs take any object: "Design Twitter's timeline".
  /^(please )?(design|architect|implement|build) \S/,
  /^let'?s (design|code|implement|build|write|create|architect|model|solve|tackle|work on)\b/,
  /^given (a|an|the|two|three|n|some|\d+)\b/,
  /^(your|the) (task|problem|challenge|question|exercise|goal) (is|here is)\b/,
  /^i('d| would) like you to\b/,
  /^(i want|we want|i need|we need) you to\b/,
];

/** Leading fillers that don't change what is asked. */
const FILLER = /^((okay|ok|so|alright|all right|right|now|next|great|cool|good|well|and)[,]?\s+)+/;

/** Meeting mechanics: questions nobody needs help answering. */
const CHIT_CHAT = [
  /^(hello|hi|hey)\b.*\?$/,
  /^can (you|everyone|everybody) (hear|see) (me|my screen|this)\b/,
  /^is my (audio|mic|microphone|video|screen) (okay|ok|working|fine|visible)\b/,
  /^are you (there|back|still there)\b/,
  /^(you're|you are) on mute\b/,
  /^is (everyone|everybody) (here|there|on|back)\b/,
  /^(can|shall|should) we (wrap|wrap up|stop|end|finish) /,
];

/** Statements that open with a question word ("What a great result"). */
const NOT_A_QUESTION_OPENER = [
  /^what an? /,
  /^how (nice|great|cool|lovely|good) /,
  /^can't /,
  /^do let /,
  /^is all /,
];

export function words(text: string): string[] {
  return text
    .toLowerCase()
    .replace(/[’`]/g, "'")
    .split(/[^a-z0-9']+/)
    .filter(Boolean);
}

function clean(text: string): string {
  return text.trim().toLowerCase().replace(/[’`]/g, "'").replace(/\s+/g, " ");
}

/** Strips a leading "Name," vocative so "Priya, what's…" reads as "what's…". */
function withoutVocative(t: string, names: string[]): string {
  const m = /^([a-z][a-z' -]{0,30}),\s*(.+)$/.exec(t);
  if (!m?.[1] || !m[2]) return t;
  const candidate = m[1];
  const looksLikeName =
    names.some((n) => similar(n, candidate)) || candidate.split(" ").length <= 2;
  return looksLikeName ? m[2] : t;
}

/** Is this utterance a question or a request to answer something? */
export function isQuestion(text: string, names: string[] = []): boolean {
  const original = clean(text);
  const t = withoutVocative(original, names);
  const addressed = t !== original;
  if (!t || CHIT_CHAT.some((r) => r.test(t))) return false;
  if (NOT_A_QUESTION_OPENER.some((r) => r.test(t))) return false;
  if (IMPLICIT.some((r) => r.test(t))) return true;
  const task = t.replace(FILLER, "");
  if (TASKS.some((r) => r.test(task)) || IMPLICIT.some((r) => r.test(task))) return true;
  // Addressed to someone and phrased as a request: "Priya, the numbers please."
  if (addressed && /\bplease[.!]?$/.test(t)) return true;
  if (/\?\s*$/.test(t))
    return words(t).length >= 2 || /^(why|how|what|who|when|where|which)\?$/.test(t);
  // STT punctuated it as a statement: trust that ("What we need is a clear owner.").
  if (/[.!]\s*$/.test(t)) return false;
  // Unpunctuated: needs a question word first and enough words to be a real question.
  const first = words(t)[0] ?? "";
  if (/^(is|are) there /.test(t)) return true;
  return (
    INTERROGATIVES.has(first) &&
    words(t).length >= 4 &&
    !/^(is|was|were|have|has|may|might|any|mind) /.test(t)
  );
}

/** Edit distance, for names that STT spells slightly differently. */
export function levenshtein(a: string, b: string): number {
  const prev = Array.from({ length: b.length + 1 }, (_, i) => i);
  for (let i = 1; i <= a.length; i++) {
    let diag = prev[0] ?? 0;
    prev[0] = i;
    for (let j = 1; j <= b.length; j++) {
      const tmp = prev[j] ?? 0;
      prev[j] = Math.min(
        (prev[j] ?? 0) + 1,
        (prev[j - 1] ?? 0) + 1,
        diag + (a[i - 1] === b[j - 1] ? 0 : 1),
      );
      diag = tmp;
    }
  }
  return prev[b.length] ?? 0;
}

/** Same name allowing small STT misspellings: none for short names, 1 edit from 5 letters, 2 from 8. */
export function similar(name: string, word: string): boolean {
  const a = name.toLowerCase();
  const b = word.toLowerCase();
  if (a === b) return true;
  const allowed = a.length >= 8 ? 2 : a.length >= 5 ? 1 : 0;
  return allowed > 0 && Math.abs(a.length - b.length) <= allowed && levenshtein(a, b) <= allowed;
}

/** The first of `names` the utterance addresses, or null. Multi-word names must match word by word. */
export function findName(text: string, names: string[]): string | null {
  const w = words(text).map((x) => x.replace(/'s$/, ""));
  for (const name of names) {
    const parts = words(name);
    if (!parts.length) continue;
    for (let i = 0; i + parts.length <= w.length; i++) {
      if (parts.every((p, k) => similar(p, w[i + k] ?? ""))) return name;
    }
  }
  return null;
}

/** Answer it? Directed questions always; others only with Auto-answer on (QA-3). */
export function shouldAnswer(q: { directed: boolean }, autoAnswer: boolean): boolean {
  return q.directed || autoAnswer;
}
