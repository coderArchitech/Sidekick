import { describe, expect, it } from "vitest";
import { highlight } from "./highlight";

describe("highlight", () => {
  it("highlights known languages and escapes the code", async () => {
    const html = await highlight('const s = "<script>";', "ts");
    expect(html).toContain("<span");
    expect(html).not.toContain("<script>");
    expect(html).toContain("&#x3C;script>");
  });

  it("returns null for unknown languages", async () => {
    expect(await highlight("x", "cobol")).toBeNull();
  });
});
