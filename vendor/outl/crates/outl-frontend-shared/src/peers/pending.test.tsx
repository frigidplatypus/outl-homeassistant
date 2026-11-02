import { render } from "solid-js/web";
import { afterEach, describe, expect, it } from "vitest";

import type { PeerStatusDto } from "../api/types";
import { PeerList } from "./PeerList";
import { pendingLabel } from "./status";

const s = (pending_ops: number | null): PeerStatusDto => ({
  node_id: "n1",
  alias: "iPhone",
  online: false,
  rtt_ms: null,
  pending_ops,
});

afterEach(() => {
  document.body.innerHTML = "";
});

describe("pendingLabel", () => {
  it("says nothing before the first exchange instead of guessing", () => {
    expect(pendingLabel(undefined)).toBeNull();
    expect(pendingLabel(s(null))).toBeNull();
  });
  it("reads up to date at zero and counts otherwise", () => {
    expect(pendingLabel(s(0))).toBe("Up to date");
    expect(pendingLabel(s(1))).toBe("1 change waiting");
    expect(pendingLabel(s(12))).toBe("12 changes waiting");
  });
});

describe("PeerList pending", () => {
  it("shows what each device still has to receive", () => {
    const host = document.createElement("div");
    document.body.appendChild(host);
    const dispose = render(
      () => (
        <PeerList
          peers={[{ node_id: "n1", alias: "iPhone", added_at: "" }]}
          statusByNodeId={new Map([["n1", s(3)]])}
        />
      ),
      host,
    );
    expect(host.querySelector(".outl-peer-list__pending")?.textContent).toBe("3 changes waiting");
    dispose();
  });
});
