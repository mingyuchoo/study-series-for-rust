import { describe, expect, it } from "vitest";
import { parseEventBlock } from "./client";

describe("parseEventBlock", () => {
  it("parses an AMAP server-sent event data payload", () => {
    const event = parseEventBlock(
      'id: 1\nevent: agent.discovery.completed\ndata: {"subject":"agent.discovery.completed","run_id":"RUN-1","function_id":"FN-1","payload":{},"at":"2026-09-04T00:00:00Z"}'
    );

    expect(event).toEqual({
      subject: "agent.discovery.completed",
      run_id: "RUN-1",
      function_id: "FN-1",
      payload: {},
      at: "2026-09-04T00:00:00Z"
    });
  });

  it("ignores keep-alive comments and malformed payloads", () => {
    expect(parseEventBlock(": keep-alive")).toBeNull();
    expect(parseEventBlock("data: not-json")).toBeNull();
  });
});
