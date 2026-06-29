export function useAutoHide(params: {
  isMenuOpenOrScrubbing: () => boolean;
  closeMenus: () => void;
  delayMs?: number;
}) {
  let controlsVisible = $state(true);
  let hideTimer: ReturnType<typeof setTimeout> | null = null;
  let lastPointerX = $state<number | null>(null);
  let lastPointerY = $state<number | null>(null);
  const delay = params.delayMs ?? 3000;

  function resetHideTimer() {
    controlsVisible = true;
    if (hideTimer) clearTimeout(hideTimer);
    hideTimer = setTimeout(() => {
      if (params.isMenuOpenOrScrubbing()) {
        resetHideTimer();
        return;
      }
      controlsVisible = false;
    }, delay);
  }

  function handleMouseMove(e: MouseEvent) {
    if (lastPointerX === e.clientX && lastPointerY === e.clientY) {
      return;
    }
    lastPointerX = e.clientX;
    lastPointerY = e.clientY;
    resetHideTimer();
  }

  function handleMouseLeave() {
    if (hideTimer) {
      clearTimeout(hideTimer);
      hideTimer = null;
    }
    params.closeMenus();
    controlsVisible = false;
    lastPointerX = null;
    lastPointerY = null;
  }

  return {
    get controlsVisible() { return controlsVisible; },
    set controlsVisible(v) { controlsVisible = v; },
    resetHideTimer,
    handleMouseMove,
    handleMouseLeave,
  };
}
