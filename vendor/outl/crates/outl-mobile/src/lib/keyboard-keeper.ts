/**
 * Keeping the iOS keyboard up across an async block create.
 *
 * WKWebView raises the software keyboard for a programmatic `.focus()`
 * only while the user-gesture window opened by the tap is still open.
 * Creating a block is two awaits deep — commit whatever draft is open,
 * then `create_block` over IPC — so by the time `<EditableTextarea>`
 * mounts and focuses itself, the gesture is long gone. The block
 * appears, the textarea really is focused (the accessibility tree says
 * `AXTextField [focused]`), and no keyboard comes up. From the user's
 * side that reads as "the + button doesn't let me type".
 *
 * The fix is a real text input that is already on the page — the
 * keeper. The tap focuses it **synchronously**, inside the gesture,
 * which raises the keyboard; the awaits then run with the keyboard
 * already up; and when the new block's textarea mounts and focuses,
 * iOS sees a move between two text inputs and leaves the keyboard
 * alone. That last part is the whole trick: a blur followed by a focus
 * in the same turn never dismisses the keyboard.
 *
 * Three properties are load-bearing, and each one is a way this breaks
 * if someone "hides it properly" later:
 *
 * - **It must be laid out.** iOS will not raise the keyboard for a
 *   `display: none`, `visibility: hidden`, `disabled` or `readonly`
 *   field. `opacity: 0` at 1×1 is the largest amount of hiding that
 *   still works.
 * - **It must live outside the outline.** Applying the new `PageView`
 *   unmounts the empty state the tap came from, so a keeper rendered
 *   inside that subtree would be destroyed mid-handoff, taking focus
 *   and the keyboard with it. Hence one node appended to
 *   `document.body`, imperatively, for the app's lifetime.
 * - **`font-size: 16px`.** Below that, focusing an input makes iOS
 *   zoom the page.
 *
 * Anything typed while the keeper holds focus lands *in the keeper*.
 * {@link takeArmedText} is how the caller carries those characters
 * into the block it just created, so the handoff window costs the user
 * nothing. The text reaches the op log through the normal commit, as an
 * edit against the id the backend issued — never against an invented
 * one.
 */

let keeper: HTMLTextAreaElement | null = null;

const KEEPER_STYLE = [
  // Fixed and inside the viewport, so iOS has nothing to scroll to.
  "position:fixed",
  "top:0",
  "left:0",
  "width:1px",
  "height:1px",
  "padding:0",
  "border:0",
  "opacity:0",
  // 16px or iOS zooms the page when this takes focus.
  "font-size:16px",
  // Focusable, never tappable: it sits over the top-left corner.
  "pointer-events:none",
].join(";");

/**
 * Put the keeper on the page if it isn't there yet. Idempotent.
 *
 * Called from the journal's `onMount` so the node is created and laid
 * out long before any tap: creating it inside the gesture would make
 * the first "+" of a session the one that has to focus an element the
 * engine has not styled yet.
 */
export function installKeyboardKeeper(): HTMLTextAreaElement | null {
  if (typeof document === "undefined") return null;
  if (keeper?.isConnected) return keeper;
  const el = document.createElement("textarea");
  el.setAttribute("style", KEEPER_STYLE);
  // Invisible to assistive tech: it is a platform workaround, not an
  // affordance, and it holds focus for one round-trip at most.
  el.setAttribute("aria-hidden", "true");
  el.tabIndex = -1;
  // Whatever lands here is carried verbatim into the new block, so the
  // keeper must not "improve" it on the way.
  el.setAttribute("autocapitalize", "off");
  el.setAttribute("autocorrect", "off");
  el.spellcheck = false;
  document.body.appendChild(el);
  keeper = el;
  return el;
}

/**
 * Raise the keyboard now, synchronously, from inside a tap handler.
 *
 * Returns whether the keeper took focus. `false` means it did not need
 * to: a text input already held focus, so the keyboard is already up
 * and stealing it would blur that textarea — which commits the draft
 * underneath the user and buys nothing.
 *
 * **Call this before the first `await` in the handler.** After an
 * await the gesture is gone and this is a no-op with extra steps.
 */
export function armKeyboard(): boolean {
  if (typeof document === "undefined") return false;
  if (holdsTextEntry(document.activeElement)) return false;
  const el = installKeyboardKeeper();
  if (!el) return false;
  el.value = "";
  el.focus({ preventScroll: true });
  return document.activeElement === el;
}

/**
 * Drain whatever the user typed while the keeper held focus, so the
 * caller can seed the new block's draft with it. Empty when the
 * keyboard was already up (the keeper never took focus) or when the
 * user typed nothing, which is the common case.
 */
export function takeArmedText(): string {
  if (!keeper) return "";
  const typed = keeper.value;
  keeper.value = "";
  return typed;
}

/**
 * Give the keyboard back. For the path where the create failed: there
 * is no textarea to hand focus to, and leaving the keeper focused
 * would leave the user typing into an invisible field.
 */
export function releaseKeyboard(): void {
  if (!keeper) return;
  keeper.value = "";
  if (document.activeElement === keeper) keeper.blur();
}

/** Whether `node` is something the soft keyboard is already up for. */
function holdsTextEntry(node: Element | null): boolean {
  if (!node) return false;
  if (node.tagName === "TEXTAREA" || node.tagName === "INPUT") return true;
  return (node as HTMLElement).isContentEditable === true;
}
