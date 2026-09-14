const AUTO_DISMISS_MS = 4000;

let container: HTMLElement | null = null;

function getContainer(): HTMLElement {
  if (container?.isConnected) return container;
  container = document.createElement("div");
  container.className = "notice-stack";
  container.setAttribute("role", "status");
  container.setAttribute("aria-live", "polite");
  document.body.appendChild(container);
  return container;
}

/** Shows a small auto-dismissing toast, bottom-left over the sidebar. */
export function showNotice(message: string): void {
  const stack = getContainer();

  const notice = document.createElement("div");
  notice.className = "notice";
  notice.textContent = message;
  stack.appendChild(notice);

  const remove = () => notice.remove();
  const timer = window.setTimeout(remove, AUTO_DISMISS_MS);
  notice.addEventListener("click", () => {
    window.clearTimeout(timer);
    remove();
  });
}

/** Logs the error and shows a user-facing Spanish notice for it. */
export function report(error: unknown, userMessage: string): void {
  console.error("[DofusTabs]", userMessage, error);
  showNotice(userMessage);
}
