import { useEffect, useState } from "react";
import { OVERLAY_CSS } from "./overlay-style";
import type { PageContextBridge } from "./types";

export function useReviewOverlayHost(
  active: boolean,
  bridge: PageContextBridge | null,
) {
  const [host, setHost] = useState<HTMLElement | null>(null);
  const [mount, setMount] = useState<HTMLElement | null>(null);

  useEffect(() => {
    if (!active || !document.body) return;
    const element = document.createElement("div");
    element.dataset.a3sTestkitOverlay = "";
    element.setAttribute("aria-label", "A3S Test review overlay");
    const shadow = element.attachShadow({ mode: "open" });
    const style = document.createElement("style");
    style.textContent = OVERLAY_CSS;
    const root = document.createElement("div");
    root.dataset.a3sTestkitOverlay = "";
    shadow.append(style, root);
    document.body.append(element);
    setHost(element);
    setMount(root);
    return () => {
      setMount(null);
      setHost(null);
      element.remove();
    };
  }, [active, bridge]);

  return { host, mount };
}
