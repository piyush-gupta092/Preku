import { describe, expect, it, vi } from "vitest";

let invokeImpl: (command: string, args?: unknown) => Promise<unknown> = async () => undefined;
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: unknown) => invokeImpl(command, args),
}));

const { call } = await import("./ipc");

describe("call", () => {
  it("wraps a successful response", async () => {
    invokeImpl = async () => ({ name: "Preku" });
    expect(await call("app_info")).toEqual({ ok: true, value: { name: "Preku" } });
  });

  it("passes through a typed backend error", async () => {
    invokeImpl = async () => {
      throw { code: "database", message: "Storage is unavailable" };
    };
    expect(await call("settings_get", { key: "x" })).toEqual({
      ok: false,
      error: { code: "database", message: "Storage is unavailable" },
    });
  });

  it("normalises unknown errors", async () => {
    invokeImpl = async () => {
      throw "boom";
    };
    expect(await call("app_info")).toEqual({
      ok: false,
      error: { code: "unknown", message: "boom" },
    });
  });
});
