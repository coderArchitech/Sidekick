import { describe, expect, it } from "vitest";
import type { AgendaItem, TranscriptLine } from "@/ipc";
import {
  agendaUpdates,
  closeAgenda,
  keywords,
  matchAgenda,
  mentions,
  parseAgendaReply,
} from "./agenda";

const items: AgendaItem[] = [
  { id: "a", text: "Pro tier pricing", status: "not_started" },
  { id: "b", text: "Dashboard performance", status: "not_started" },
  { id: "c", text: "Hiring plan", status: "not_started" },
];
const [A, B, C] = items as [AgendaItem, AgendaItem, AgendaItem];
const line = (text: string, endMs = 1000): TranscriptLine => ({
  id: endMs,
  speakerLabel: "s",
  isUser: false,
  text,
  startMs: endMs - 500,
  endMs,
});

describe("agenda keywords", () => {
  it("drops filler and plural s", () => {
    expect(keywords("Review the Q3 dashboards and metrics")).toEqual(["q3", "dashboard", "metric"]);
  });

  it("an item is mentioned when half its keywords are said", () => {
    expect(mentions("What's the rate limit for the Pro tier?", "Pro tier pricing")).toBe(true);
    expect(mentions("Pricing is fine", "Pro tier pricing")).toBe(false);
    expect(mentions("The dashboard is slow", "Dashboard performance")).toBe(true);
    expect(mentions("anything", "the and")).toBe(false);
  });
});

describe("matchAgenda", () => {
  it("the last item mentioned is in progress, earlier ones covered", () => {
    const m = matchAgenda(items, [
      line("Pro tier first", 1),
      line("Now the dashboard", 2),
      line("Thanks all", 3),
    ]);
    expect(Object.fromEntries(m)).toEqual({ a: "covered", b: "in_progress" });
  });
});

describe("parseAgendaReply", () => {
  it("maps item numbers to ids and ignores bad entries", () => {
    const m = parseAgendaReply(
      'Sure: {"agenda": [{"n": 1, "status": "covered"}, {"n": 9, "status": "covered"}, {"n": 2, "status": "done"}, "x"]}',
      items,
    );
    expect(Object.fromEntries(m ?? [])).toEqual({ a: "covered" });
  });

  it("returns null for prose, bad JSON or no usable entries", () => {
    expect(parseAgendaReply("Mock answer to: <agenda>", items)).toBeNull();
    expect(parseAgendaReply("{not json}", items)).toBeNull();
    expect(parseAgendaReply('{"agenda": []}', items)).toBeNull();
    expect(parseAgendaReply('{"other": 1}', items)).toBeNull();
  });
});

describe("agendaUpdates", () => {
  it("statuses only move forward", () => {
    const current: AgendaItem[] = [{ ...A, status: "covered" }, { ...B, status: "in_progress" }, C];
    const u = agendaUpdates(
      current,
      new Map([
        ["a", "not_started"],
        ["b", "not_started"],
      ]),
    );
    expect(u).toEqual([]);
  });

  it("starting an item covers the one in progress", () => {
    const current: AgendaItem[] = [{ ...A, status: "in_progress" }, B, C];
    expect(agendaUpdates(current, new Map([["c", "in_progress"]]))).toEqual([
      { id: "a", status: "covered" },
      { id: "c", status: "in_progress" },
    ]);
  });

  it("closeAgenda covers what was in progress", () => {
    expect(closeAgenda([{ ...A, status: "in_progress" }, B])).toEqual([
      { id: "a", status: "covered" },
    ]);
  });
});
