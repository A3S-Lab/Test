import {
  ArrowClockwise,
  CheckCircle,
  Code,
  CursorClick,
  Package,
  PaperPlaneTilt,
  Pause,
  Play,
  Scan,
  ShieldCheck,
  ShoppingCartSimple,
} from '@phosphor-icons/react';
import { getPageContextBridge } from '@a3s-lab/testkit';
import {
  A3SReviewOverlay,
  A3STestBoundary,
  A3STestKit,
} from '@a3s-lab/testkit/react';
import { withBase } from '@rspress/core/runtime';
import { useEffect, useRef, useState } from 'react';
import type {
  ContextNode,
  LocatorCandidate,
  SubmittedRepair,
} from '@a3s-lab/testkit';
import type { ExperienceCopy, Locale } from '../home-copy';

type ContextView = {
  nodeId: string;
  revision: number;
  role: string;
  name: string;
  geometry: string;
  locator: string;
  source: string;
};

function locatorText(locator: LocatorCandidate | undefined) {
  if (!locator) return 'n/a';
  if (locator.type === 'role') {
    return `role=${locator.role} · name=${JSON.stringify(locator.name)}`;
  }
  if (locator.type === 'text') {
    return `text=${JSON.stringify(locator.value)}`;
  }
  return `${locator.type}=${JSON.stringify(locator.value)}`;
}

function geometryText(node: ContextNode) {
  const bounds = node.geometry?.viewport;
  if (!bounds) return 'n/a';
  return [
    `x ${Math.round(bounds.x)}`,
    `y ${Math.round(bounds.y)}`,
    `w ${Math.round(bounds.width)}`,
    `h ${Math.round(bounds.height)}`,
  ].join(' · ');
}

function LiveContextPanel({
  copy,
  refreshKey,
  onRefresh,
}: {
  copy: ExperienceCopy;
  refreshKey: number;
  onRefresh: () => void;
}) {
  const [context, setContext] = useState<ContextView | null>(null);

  useEffect(() => {
    let cancelled = false;
    let timer = 0;
    const deadline = Date.now() + 10_000;

    const capture = () => {
      if (cancelled) return;
      const bridge = getPageContextBridge();
      const snapshot = bridge?.snapshot({
        detail: 'forensic',
        limits: { nodes: 1500 },
      });
      const node = snapshot?.nodes.find(
        (candidate) => candidate.testId === 'a3s-experience-submit',
      );

      if (!snapshot || !node) {
        if (Date.now() < deadline) timer = window.setTimeout(capture, 50);
        return;
      }

      const component = snapshot.components.find(
        (candidate) => candidate.id === node.componentId,
      );
      const source = component?.source;
      const sourceLabel = source
        ? `${source.file}${source.line ? `:${source.line}` : ''}`
        : (component?.name ?? 'n/a');
      setContext({
        nodeId: node.id,
        revision: snapshot.revision,
        role: node.role ?? node.tag,
        name: node.name ?? node.text ?? 'n/a',
        geometry: geometryText(node),
        locator: locatorText(node.locators[0]),
        source: sourceLabel,
      });
    };

    timer = window.setTimeout(capture, 0);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [refreshKey]);

  return (
    <section className="test-context-panel" aria-labelledby="context-title">
      <header>
        <div>
          <Code aria-hidden="true" size={17} weight="bold" />
          <h2 id="context-title">{copy.contextTitle}</h2>
        </div>
        <button onClick={onRefresh} type="button">
          <ArrowClockwise aria-hidden="true" size={14} weight="bold" />
          {copy.refresh}
        </button>
      </header>
      {context ? (
        <dl className="test-context-data">
          <div className="test-context-target">
            <dt>{copy.selected}</dt>
            <dd>
              <code>@{context.nodeId}</code>
              <span>{context.role}</span>
            </dd>
          </div>
          <div className="test-context-split">
            <div>
              <dt>{copy.revision}</dt>
              <dd>{context.revision}</dd>
            </div>
            <div>
              <dt>{copy.role}</dt>
              <dd>{context.role}</dd>
            </div>
          </div>
          <div>
            <dt>{copy.name}</dt>
            <dd>{context.name}</dd>
          </div>
          <div>
            <dt>{copy.geometry}</dt>
            <dd className="test-context-geometry">
              <span aria-hidden="true">
                <i />
              </span>
              <code>{context.geometry}</code>
            </dd>
          </div>
          <div>
            <dt>{copy.locator}</dt>
            <dd>
              <code>{context.locator}</code>
            </dd>
          </div>
          <div>
            <dt>{copy.source}</dt>
            <dd>
              <code>{context.source}</code>
            </dd>
          </div>
        </dl>
      ) : (
        <p className="test-context-loading" role="status">
          {copy.connecting}
        </p>
      )}
    </section>
  );
}

function CheckoutSurface({
  confirmed,
  copy,
  onConfirm,
}: {
  confirmed: boolean;
  copy: ExperienceCopy;
  onConfirm: () => void;
}) {
  return (
    <A3STestBoundary
      as="section"
      className="test-demo-page"
      facts={() => ({
        data: 'illustrative',
        orderStatus: confirmed ? 'submitted' : 'review',
      })}
      id="checkout-experience"
      name="Checkout experience"
      source={{
        file: 'website/theme/components/TestKitExperience.tsx',
        line: 179,
      }}
    >
      <header className="test-demo-nav">
        <strong>
          <img alt="" height="24" src={withBase('/a3s-logo.png')} width="24" />
          {copy.shop}
        </strong>
        <span>{copy.sample}</span>
        <ShoppingCartSimple aria-hidden="true" size={18} weight="bold" />
      </header>
      <div className="test-demo-content">
        <div className="test-demo-heading">
          <h2>{copy.checkoutTitle}</h2>
          <span className="test-sample-badge">{copy.sample}</span>
        </div>
        <ol className="test-checkout-progress" aria-label={copy.checkoutTitle}>
          {copy.checkoutProgress.map((step, index) => (
            <li className={index === 1 ? 'is-current' : undefined} key={step}>
              <span>{index + 1}</span>
              {step}
            </li>
          ))}
        </ol>
        <div className="test-checkout-layout">
          <div className="test-checkout-main">
            <section className="test-checkout-section">
              <header>
                <h3>{copy.customerTitle}</h3>
                <ShieldCheck aria-hidden="true" size={17} weight="fill" />
              </header>
              <strong>{copy.customerName}</strong>
              <p>{copy.customerAddress}</p>
            </section>
            <section className="test-checkout-section test-order-items">
              <header>
                <h3>{copy.productsTitle}</h3>
                <span>{copy.quantity}</span>
              </header>
              <div className="test-order-item">
                <span className="test-product-icon" aria-hidden="true">
                  <Package size={22} weight="duotone" />
                </span>
                <div>
                  <strong>{copy.productName}</strong>
                  <small>{copy.productVariant}</small>
                </div>
                <b>{copy.subtotal}</b>
              </div>
            </section>
          </div>
          <aside className="test-order-summary" aria-labelledby="summary-title">
            <h3 id="summary-title">{copy.summaryTitle}</h3>
            <dl>
              <div>
                <dt>{copy.productTotal}</dt>
                <dd>{copy.subtotal}</dd>
              </div>
              <div>
                <dt>{copy.delivery}</dt>
                <dd>{copy.deliveryValue}</dd>
              </div>
              <div>
                <dt>{copy.discount}</dt>
                <dd>{copy.discountValue}</dd>
              </div>
              <div className="test-order-total">
                <dt>{copy.payable}</dt>
                <dd>{copy.subtotal}</dd>
              </div>
            </dl>
            <fieldset>
              <legend>{copy.paymentTitle}</legend>
              <label>
                <input defaultChecked name="demo-payment" type="radio" />
                {copy.paymentMethod}
              </label>
            </fieldset>
            <div className="test-selected-action">
              <span aria-hidden="true">target</span>
              <button
                aria-pressed={confirmed}
                data-testid="a3s-experience-submit"
                onClick={onConfirm}
                type="button"
              >
                {confirmed ? (
                  <CheckCircle aria-hidden="true" size={18} weight="fill" />
                ) : null}
                {copy.submit}
              </button>
            </div>
            <p aria-live="polite" className="test-demo-status">
              {confirmed ? copy.submitted : ''}
            </p>
          </aside>
        </div>
      </div>
    </A3STestBoundary>
  );
}

export function TestKitExperience({
  copy,
  locale,
  reviewStarted,
  onStartReview,
}: {
  copy: ExperienceCopy;
  locale: Locale;
  reviewStarted: boolean;
  onStartReview: () => void;
}) {
  const [confirmed, setConfirmed] = useState(false);
  const [refreshKey, setRefreshKey] = useState(0);
  const [repairs, setRepairs] = useState<SubmittedRepair[]>([]);
  const [motionActive, setMotionActive] = useState(false);
  const [motionPaused, setMotionPaused] = useState(false);
  const [motionStep, setMotionStep] = useState(0);
  const stageRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const stage = stageRef.current;
    if (!stage || typeof IntersectionObserver === 'undefined') return;
    const setActive = (active: boolean) => {
      setMotionActive(active);
      if (!active) setMotionStep(0);
    };
    const rect = stage.getBoundingClientRect();
    const visibleWidth = Math.max(
      0,
      Math.min(rect.right, window.innerWidth) - Math.max(rect.left, 0),
    );
    const visibleHeight = Math.max(
      0,
      Math.min(rect.bottom, window.innerHeight) - Math.max(rect.top, 0),
    );
    const area = rect.width * rect.height;
    setActive(area > 0 && (visibleWidth * visibleHeight) / area >= 0.28);
    const observer = new IntersectionObserver(
      ([entry]) => {
        setActive(Boolean(entry?.isIntersecting));
      },
      { threshold: 0.28 },
    );
    observer.observe(stage);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (
      !motionActive ||
      motionPaused ||
      reviewStarted ||
      window.matchMedia('(prefers-reduced-motion: reduce), (max-width: 768px)')
        .matches
    ) {
      return;
    }
    const interval = window.setInterval(() => {
      setMotionStep((current) => (current + 1) % copy.motionSteps.length);
    }, 2600);
    return () => window.clearInterval(interval);
  }, [copy.motionSteps.length, motionActive, motionPaused, reviewStarted]);

  const refreshAfterRender = () => {
    window.requestAnimationFrame(() => {
      setRefreshKey((current) => current + 1);
    });
  };

  return (
    <section
      aria-label={copy.stageAria}
      className="test-experience-shell"
      id="testkit-experience"
    >
      <A3STestKit
        enabled
        facts={() => ({ demo: true, locale, persistence: 'memory' })}
        maxNodes={1500}
        page={{ id: `a3s-test-home-${locale}` }}
        redact={['[data-demo-private]']}
        repairStorage="memory"
      >
        <div
          className={`test-experience-stage is-motion-step-${motionStep}${motionActive && !reviewStarted ? ' is-motion-active' : ''}`}
          data-motion-step={motionStep + 1}
          ref={stageRef}
        >
          <CheckoutSurface
            confirmed={confirmed}
            copy={copy}
            onConfirm={() => {
              setConfirmed(true);
              refreshAfterRender();
            }}
          />
          <LiveContextPanel
            copy={copy}
            onRefresh={() => setRefreshKey((current) => current + 1)}
            refreshKey={refreshKey}
          />
          <aside className="test-review-rail">
            <section className="test-review-panel">
              <header>
                <div>
                  <ShieldCheck aria-hidden="true" size={17} weight="bold" />
                  <h2>{copy.reviewTitle}</h2>
                </div>
                <span>{reviewStarted ? 'LIVE' : copy.sample}</span>
              </header>
              <p>{copy.reviewBody}</p>
              <button
                className="test-review-start"
                disabled={reviewStarted}
                onClick={onStartReview}
                type="button"
              >
                {reviewStarted ? copy.reviewStarted : copy.openReview}
              </button>
              <small>{copy.localOnly}</small>
            </section>
            <section
              aria-live="polite"
              className={`test-evidence-receipt${repairs.length ? ' is-ready' : ''}`}
            >
              <header>
                <div>
                  <CheckCircle aria-hidden="true" size={17} weight="fill" />
                  <h2>{copy.evidenceTitle}</h2>
                </div>
                <span>
                  {repairs.length ? copy.evidenceReady : copy.evidenceWaiting}
                </span>
              </header>
              <dl>
                <div>
                  <dt>ID</dt>
                  <dd>{repairs.at(-1)?.id.slice(0, 16) ?? 'n/a'}</dd>
                </div>
                <div>
                  <dt>STATUS</dt>
                  <dd>{repairs.length ? 'memory' : 'idle'}</dd>
                </div>
                <div>
                  <dt>FINDINGS</dt>
                  <dd>
                    {repairs.length
                      ? `${repairs.length} ${copy.findingsUnit}`
                      : copy.noFinding}
                  </dd>
                </div>
              </dl>
            </section>
          </aside>
          <span className="test-evidence-path" aria-hidden="true" />
          <div className="test-product-motion" aria-hidden="true">
            <div className="test-motion-scan">
              <i />
              <Scan size={14} weight="bold" />
              <span>DOM · A11Y · XY</span>
            </div>
            <div className="test-motion-context">
              <header>
                <Code size={13} weight="bold" />
                <span>{copy.motionContext}</span>
              </header>
              <strong>{copy.motionContextValue}</strong>
              <code>role=button</code>
              <code>x=940 y=566 w=167 h=38</code>
            </div>
            <CursorClick
              className="test-motion-cursor"
              size={22}
              weight="fill"
            />
            <div className="test-motion-note">
              <header>
                <span>01</span>
                <div>
                  <strong>{copy.motionFinding}</strong>
                  <small>{copy.motionContextValue}</small>
                </div>
              </header>
              <p>{copy.motionRequest}</p>
              <footer>
                <span>{copy.motionAdd}</span>
                <strong>{copy.motionSend}</strong>
              </footer>
            </div>
            <div className="test-motion-packet">
              <PaperPlaneTilt size={14} weight="fill" />
              <span>{copy.motionPacket}</span>
            </div>
            <div className="test-motion-receipt">
              <CheckCircle size={16} weight="fill" />
              <span>
                <strong>{copy.motionReady}</strong>
                <small>{copy.motionPacket}</small>
              </span>
            </div>
          </div>
        </div>
        <div className={`test-stage-timeline is-motion-step-${motionStep}`}>
          <ol className="test-stage-status" aria-label={copy.stageAria}>
            {copy.motionSteps.map((step, index) => (
              <li
                aria-current={
                  motionActive && !reviewStarted && index === motionStep
                    ? 'step'
                    : undefined
                }
                className={[
                  motionActive && !reviewStarted && index === motionStep
                    ? 'is-current'
                    : '',
                  motionActive && !reviewStarted && index < motionStep
                    ? 'is-complete'
                    : '',
                  repairs.length && index === 4 ? 'is-ready' : '',
                ]
                  .filter(Boolean)
                  .join(' ')}
                key={step}
              >
                <span>{index + 1}</span>
                {step}
              </li>
            ))}
          </ol>
          {!reviewStarted ? (
            <button
              aria-label={motionPaused ? copy.motionResume : copy.motionPause}
              aria-pressed={motionPaused}
              className="test-motion-toggle"
              onClick={() => setMotionPaused((current) => !current)}
              title={motionPaused ? copy.motionResume : copy.motionPause}
              type="button"
            >
              {motionPaused ? (
                <Play aria-hidden="true" size={13} weight="fill" />
              ) : (
                <Pause aria-hidden="true" size={13} weight="fill" />
              )}
            </button>
          ) : null}
        </div>
        {reviewStarted ? (
          <A3SReviewOverlay
            defaultOpen
            enabled
            locale={locale === 'zh' ? 'zh-CN' : 'en'}
            onSubmitted={(submitted) => {
              setRepairs(submitted);
              refreshAfterRender();
            }}
          />
        ) : null}
      </A3STestKit>
    </section>
  );
}
