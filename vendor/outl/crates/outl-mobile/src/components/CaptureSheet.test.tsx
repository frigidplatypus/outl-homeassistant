/**
 * The capture sheet closes with the same drag-down gesture as every
 * other bottom sheet (#362, issue #364: consistent gestures).
 */
import { render } from "solid-js/web";
import { afterEach, describe, expect, it, vi } from "vitest";

import { CaptureSheet } from "./CaptureSheet";

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  document.body.innerHTML = "";
});

function mount() {
  const host = document.createElement("div");
  document.body.appendChild(host);
  const onClose = vi.fn();
  dispose = render(
    () => <CaptureSheet open target="Today" onClose={onClose} onSubmit={async () => true} />,
    host,
  );
  return { onClose, handle: document.querySelector<HTMLElement>('[aria-label="Drag to close"]') };
}

const pointer = (type: string, clientY: number) =>
  new MouseEvent(type, { bubbles: true, clientY }) as unknown as PointerEvent;

describe("CaptureSheet drag", () => {
  it("has a grab handle and closes past the threshold", () => {
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
