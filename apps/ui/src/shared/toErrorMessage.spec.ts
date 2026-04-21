import { describe, expect, it } from "vitest";
import { toErrorMessage } from "./toErrorMessage";

describe("toErrorMessage", () => {
  it("returns the original string for Tauri backend-rejected strings", () => {
    expect(toErrorMessage("授权服务不可用")).toBe("授权服务不可用");
    expect(toErrorMessage("")).toBe("");
  });

  it("prefers Error.message over default stringification", () => {
    const err = new Error("network timeout");
    expect(toErrorMessage(err)).toBe("network timeout");
  });

  it("preserves custom Error subclasses' message", () => {
    class MyCustomError extends Error {}
    const err = new MyCustomError("custom");
    expect(toErrorMessage(err)).toBe("custom");
  });

  it("falls back to String() for non-string / non-Error values", () => {
    expect(toErrorMessage(42)).toBe("42");
    expect(toErrorMessage(null)).toBe("null");
    expect(toErrorMessage(undefined)).toBe("undefined");
    expect(toErrorMessage({ code: "E01" })).toBe("[object Object]");
  });
});
