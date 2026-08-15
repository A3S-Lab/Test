import {
  ArrowClockwise,
  CheckCircle,
  Code,
  Package,
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
import { useEffect, useState } from 'react';
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
    let frame = 0;
    let remainingFrames = 120;

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
        remainingFrames -= 1;
        if (remainingFrames > 0) frame = window.requestAnimationFrame(capture);
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

    frame = window.requestAnimationFrame(capture);
    return () => {
      cancelled = true;
      window.cancelAnimationFrame(frame);
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
        <div className="test-experience-stage">
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
        </div>
        <ol className="test-stage-status" aria-label={copy.stageAria}>
          <li>
            <span />
            {copy.renderedStatus}
          </li>
          <li>
            <span />
            {copy.contextStatus}
          </li>
          <li className={repairs.length ? 'is-ready' : undefined}>
            <span />
            {copy.evidenceStatus}
          </li>
        </ol>
        {reviewStarted ? (
          <A3SReviewOverlay
            defaultOpen
            enabled
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
