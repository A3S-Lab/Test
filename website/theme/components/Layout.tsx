import { useLocation } from '@rspress/core/runtime';
import {
  Layout as OriginalLayout,
  type LayoutProps,
} from '@rspress/core/theme-original';
import { useEffect } from 'react';

const DIRECTION_CONTRACT = `<!--
THESIS: A3S Test makes the evidence path visible in one operating surface and refuses the generic developer-tool hero followed by a card grid.
OWN-WORLD: A bright cool canvas, electric control blue, precise white interface planes, cool hairlines, compact enterprise controls, and green or violet only for evidence or review authority.
STORY: Visitors see a rendered page become revisioned context, open real human review, then install the CLI or enter the documentation.
FIRST VIEWPORT: A shallow two-column promise band leads into a 56/24/20 rendered-page, context, and review/evidence stage; the primary experience action sits beside the promise and repeats inside review.
FORM: Wide Evidence Stage, composition 2 of 3, seed bf8ff2ac.
FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, and DESIGN.md
-->`;

const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[contenteditable="true"]',
  '[tabindex]',
].join(',');

const PREVIOUS_TABINDEX = 'data-a3s-previous-tabindex';
const NO_TABINDEX = '__none__';

function setSidebarAvailable(sidebar: HTMLElement, available: boolean) {
  sidebar.toggleAttribute('inert', !available);
  if (available) {
    sidebar.removeAttribute('aria-hidden');
  } else {
    sidebar.setAttribute('aria-hidden', 'true');
  }

  sidebar
    .querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)
    .forEach((element) => {
      if (!available) {
        if (!element.hasAttribute(PREVIOUS_TABINDEX)) {
          element.setAttribute(
            PREVIOUS_TABINDEX,
            element.getAttribute('tabindex') ?? NO_TABINDEX,
          );
        }
        element.tabIndex = -1;
        return;
      }

      const previous = element.getAttribute(PREVIOUS_TABINDEX);
      if (previous === null) return;
      if (previous === NO_TABINDEX) {
        element.removeAttribute('tabindex');
      } else {
        element.setAttribute('tabindex', previous);
      }
      element.removeAttribute(PREVIOUS_TABINDEX);
    });
}

function useAccessibleMobileSidebar() {
  const { pathname } = useLocation();

  useEffect(() => {
    const mobile = window.matchMedia('(max-width: 768px)');
    const sync = () => {
      document
        .querySelectorAll<HTMLElement>('.rp-doc-layout__sidebar')
        .forEach((sidebar) => {
          const open = sidebar.classList.contains(
            'rp-doc-layout__sidebar--open',
          );
          setSidebarAvailable(sidebar, !mobile.matches || open);
        });
    };

    const observer = new MutationObserver(sync);
    observer.observe(document.body, {
      attributeFilter: ['class'],
      attributes: true,
      childList: true,
      subtree: true,
    });
    mobile.addEventListener('change', sync);
    sync();

    return () => {
      observer.disconnect();
      mobile.removeEventListener('change', sync);
      document
        .querySelectorAll<HTMLElement>('.rp-doc-layout__sidebar')
        .forEach((sidebar) => setSidebarAvailable(sidebar, true));
    };
  }, [pathname]);
}

export function Layout(props: LayoutProps) {
  useAccessibleMobileSidebar();

  return (
    <>
      {!import.meta.env.SSG_MD && (
        <template
          data-impeccable-contract="bf8ff2ac"
          dangerouslySetInnerHTML={{ __html: DIRECTION_CONTRACT }}
        />
      )}
      <OriginalLayout {...props} />
    </>
  );
}
