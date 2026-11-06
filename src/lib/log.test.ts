import { afterEach, describe, expect, it } from "vitest";
import { mockLogs, resetMocks } from "@/ipc/testing";
import { describeError, installErrorLogging, log, resetErrorLogging } from "./log";

const tick = () => new Promise((r) => setTimeout(r, 0));

describe("frontend logging", () => {
  afterEach(() => {
    resetMocks();
    resetErrorLogging();
  });

  it("sends level, message and ids to the log command", async () => {
    log.warn("answer failed: key missing", { requestId: "a1", sessionId: "s1" });
    log.info("x".repeat(900));
    await tick();
    expect(mockLogs[0]).toEqual({
      level: "warn",
      message: "answer failed: key missing",
      requestId: "a1",
    });
    expect(mockLogs[1]?.message).toHaveLength(500);
  });

  it("describes errors without their messages, which may hold content", () => {
    const e = new TypeError("transcript said: our secret plan");
    const d = describeError(e);
    expect(d.startsWith("TypeError")).toBe(true);
    expect(d).not.toContain("secret plan");
    expect(describeError("oops")).toBe("string");
  });

  it("forwards uncaught errors and rejections", async () => {
    const target = new EventTarget() as unknown as Window;
    installErrorLogging(target);
    target.dispatchEvent(Object.assign(new Event("error"), { error: new RangeError("content") }));
    target.dispatchEvent(
      Object.assign(new Event("unhandledrejection"), { reason: new Error("x") }),
    );
    await tick();
    expect(mockLogs.map((l) => l.message.split(" ").slice(0, 2).join(" "))).toEqual([
      "uncaught: RangeError",
      "unhandled rejection:",
    ]);
    expect(mockLogs.every((l) => l.level === "error" && !l.message.includes("content"))).toBe(true);
  });
});
