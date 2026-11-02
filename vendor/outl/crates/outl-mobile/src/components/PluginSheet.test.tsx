/**
 * The plugin sheet closes with the same drag-down gesture as every other
 * bottom sheet (#364: consistent gestures).
 */
import { render } from "solid-js/web";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => []) }));

import { PluginSheet } from "./PluginSheet";

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  document.body.innerHTML = "";
});

function mount() {
  const onClose = vi.fn();
  const host = document.createElement("div");
  document.body.appendChild(host);
  dispose = render(
    () => (
      <PluginSheet open pageId={null} onClose={onClose} onMessage={() => {}} onView={() => {}} onViews={() => {}} />
    ),
    host,
  );
  return { onClose, handle: document.querySelector<HTMLElement>('[aria-label="Drag to close"]') };
}

const pointer = (type: string, clientY: number) =>
  new MouseEvent(type, { bubbles: true, clientY }) as unknown as PointerEvent;

describe("PluginSheet drag", () => {
  it("closes past the threshold", () => {
    const { onClose, handle } = mount();
    expect(handle).not.toBeNull();
    handle!.dispatchEvent(pointer("pointerdown", 100));
    handle!.dispatchEvent(pointer("pointermove", 300));
    handle!.dispatchEvent(pointer("pointerup", 300));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("springs back on a short drag", () => {
    const { onClose, handle } = mount();
    handle!.dispatchEvent(pointer("pointerdown", 100));
    handle!.dispatchEvent(pointer("pointermove", 130));
    handle!.dispatchEvent(pointer("pointerup", 130));
    expect(onClose).not.toHaveBeenCalled();
  });
});
