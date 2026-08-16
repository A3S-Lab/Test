import { ArrowRight, CheckCircle } from '@phosphor-icons/react';

type Labels = {
  contractPanelLabel: string;
  contractExpectedLabel: string;
  contractObservedLabel: string;
  contractPrdTitle: string;
  contractPrdBody: string;
  contractDesignTitle: string;
  contractDesignBody: string;
  contractPageTitle: string;
  contractPageBody: string;
  contractReviewTitle: string;
  contractReviewBody: string;
  contractCompareTitle: string;
  contractCompareBody: string;
  contractReportTitle: string;
  contractReportBody: string;
  contractDisclaimer: string;
};

export function ContractPanel({ labels }: { labels: Labels }) {
  return (
    <figure className="test-contract-panel">
      <figcaption>
        <span>{labels.contractPanelLabel}</span>
        <code>surface-contract / checkout</code>
      </figcaption>
      <div className="test-contract-sources">
        <article>
          <code>{labels.contractExpectedLabel} · PRD</code>
          <strong>{labels.contractPrdTitle}</strong>
          <p>{labels.contractPrdBody}</p>
        </article>
        <article>
          <code>{labels.contractExpectedLabel} · DESIGN</code>
          <strong>{labels.contractDesignTitle}</strong>
          <p>{labels.contractDesignBody}</p>
        </article>
        <article>
          <code>{labels.contractObservedLabel} · PAGE CONTEXT</code>
          <strong>{labels.contractPageTitle}</strong>
          <p>{labels.contractPageBody}</p>
        </article>
      </div>
      <div className="test-contract-flow">
        <article>
          <span>1</span>
          <strong>{labels.contractReviewTitle}</strong>
          <small>{labels.contractReviewBody}</small>
        </article>
        <ArrowRight aria-hidden="true" size={18} weight="bold" />
        <article>
          <span>2</span>
          <strong>{labels.contractCompareTitle}</strong>
          <small>{labels.contractCompareBody}</small>
        </article>
        <ArrowRight aria-hidden="true" size={18} weight="bold" />
        <article className="is-result">
          <CheckCircle aria-hidden="true" size={18} weight="fill" />
          <strong>{labels.contractReportTitle}</strong>
          <small>{labels.contractReportBody}</small>
        </article>
      </div>
      <p className="test-contract-disclaimer">{labels.contractDisclaimer}</p>
    </figure>
  );
}
