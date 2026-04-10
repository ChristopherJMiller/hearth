/**
 * Shared body-scroll lock used by Sheet, Modal, and the command palette.
 * Reference-counted so stacked overlays don't clobber each other's
 * `overflow: hidden` toggle — when the last caller releases, we restore the
 * original body overflow exactly once.
 */
let count = 0;
let previousOverflow = "";

export function lockBodyScroll(): () => void {
  if (count === 0) {
    previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
  }
  count += 1;

  let released = false;
  return () => {
    if (released) return;
    released = true;
    count -= 1;
    if (count === 0) {
      document.body.style.overflow = previousOverflow;
    }
  };
}
