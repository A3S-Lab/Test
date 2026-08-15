import type { Locale } from '../home-copy';

type Labels = {
  sampleLabel: string;
  observationLabel: string;
  revisionLabel: string;
  actionLabel: string;
  evidenceLabel: string;
  pageContextLabel: string;
  pageContextValue: string;
  actionValue: string;
  evidenceValue: string;
};

export function EvidencePanel({
  labels,
  locale,
}: {
  labels: Labels;
  locale: Locale;
}) {
  return (
    <figure className="test-evidence-panel">
      <figcaption>
        <span>{labels.sampleLabel}</span>
        <span>checkout / ready</span>
      </figcaption>
      <div className="test-evidence-observation">
        <div>
          <span>{labels.observationLabel}</span>
          <strong>42</strong>
        </div>
        <div>
          <span>{labels.revisionLabel}</span>
          <strong>7</strong>
        </div>
      </div>
      <div className="test-tree" lang="en">
        <div className="test-tree-row test-tree-row-root">
          <code>@c1</code>
          <span>main</span>
          <strong>Checkout</strong>
        </div>
        <div className="test-tree-row">
          <code>@c4</code>
          <span>form</span>
          <strong>Payment details</strong>
        </div>
        <div className="test-tree-row is-target">
          <code>@c7</code>
          <span>button</span>
          <strong>Place order</strong>
          <small>x 1048&nbsp; y 716&nbsp; w 168&nbsp; h 44</small>
        </div>
      </div>
      <dl className="test-evidence-ledger">
        <div>
          <dt>{labels.pageContextLabel}</dt>
          <dd>{labels.pageContextValue}</dd>
        </div>
        <div>
          <dt>{labels.actionLabel}</dt>
          <dd>{labels.actionValue}</dd>
        </div>
        <div>
          <dt>{labels.evidenceLabel}</dt>
          <dd>{labels.evidenceValue}</dd>
        </div>
      </dl>
      <p className="test-sample-disclaimer">
        {locale === 'zh'
          ? '示例数据，用于说明上下文与动作绑定。'
          : 'Sample data illustrating context-to-action binding.'}
      </p>
    </figure>
  );
}
