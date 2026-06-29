type CloseFn = () => void;

let activeMenuCloser = $state<CloseFn | null>(null);

export function registerMenu(closeFn: CloseFn) {
  if (activeMenuCloser && activeMenuCloser !== closeFn) {
    try {
      const prev = activeMenuCloser;
      activeMenuCloser = null;
      prev();
    } catch (e) {
      console.warn("Failed to close previous menu:", e);
    }
  }
  activeMenuCloser = closeFn;
}

export function closeActiveMenu() {
  if (activeMenuCloser) {
    try {
      const prev = activeMenuCloser;
      activeMenuCloser = null;
      prev();
    } catch (e) {
      console.warn("Failed to close active menu:", e);
    }
  }
}
