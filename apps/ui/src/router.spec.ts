import { describe, expect, it } from "vitest";
import { routes } from "./routeRecords";

describe("router", () => {
  it("redirects the legacy license route into settings license section", () => {
    const licenseRoute = routes.find((route) => route.path === "/license");

    expect(licenseRoute).toBeDefined();
    expect(licenseRoute?.redirect).toEqual({
      name: "settings",
      query: { section: "license" },
    });
  });
});
