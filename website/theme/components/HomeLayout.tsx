import { useLang, useSite, useVersion, withBase } from '@rspress/core/runtime';
import { ArrowRight, ArrowUpRight, CaretDown } from '@phosphor-icons/react';
import { useState } from 'react';
import { ContractPanel } from './ContractPanel';
import { InstallSwitcher, installCommandFor } from './InstallSwitcher';
import { TestKitExperience } from './TestKitExperience';
import { homeCopy, type CapabilityGroupId, type Locale } from '../home-copy';
import { publishedVersion } from '../../versions.mjs';

const archivedCapabilityHrefs: Record<CapabilityGroupId, string> = {
  context: '/guide/testkit.html',
  safety: '/concepts/architecture.html',
  repair: '/guide/testkit.html',
  execution: '/reference/cli.html',
  contracts: '/guide/contracts.html',
  evidence: '/guide/workflows.html',
};

function MarkdownHome({
  installVersion,
  locale,
  version,
}: {
  installVersion: string;
  locale: Locale;
  version: string;
}) {
  const copy = homeCopy[locale];
  const unixInstall = installCommandFor('macos', installVersion);
  const windowsInstall = installCommandFor('windows', installVersion);

  return (
    <main>
      <h1>{copy.heroTitle.join(locale === 'zh' ? '' : ' ')}</h1>
      <p>{copy.heroBody}</p>
      <h2>{copy.installTitle}</h2>
      {version !== installVersion && <p>{copy.installCandidateNote}</p>}
      <p>{copy.testkitInstallLink}</p>
      <h3>macOS / Linux</h3>
      <pre>
        <code>{unixInstall}</code>
      </pre>
      <h3>Windows PowerShell</h3>
      <pre>
        <code>{windowsInstall}</code>
      </pre>
      <h2>{copy.proofTitle}</h2>
      <p>{copy.proofBody}</p>
      <h3>PRD · {copy.contractPrdTitle}</h3>
      <p>{copy.contractPrdBody}</p>
      <h3>Design · {copy.contractDesignTitle}</h3>
      <p>{copy.contractDesignBody}</p>
      <h3>Page Context · {copy.contractPageTitle}</h3>
      <p>{copy.contractPageBody}</p>
      <h3>{copy.contractReviewTitle}</h3>
      <p>{copy.contractReviewBody}</p>
      <h3>{copy.contractCompareTitle}</h3>
      <p>{copy.contractCompareBody}</p>
      <h3>{copy.contractReportTitle}</h3>
      <p>{copy.contractReportBody}</p>
      <p>{copy.contractDisclaimer}</p>
      <h2>{copy.experience.contextTitle}</h2>
      <p>{copy.experience.localOnly}</p>
      <h2>{copy.capabilitiesTitle}</h2>
      {copy.capabilities.map((item) => (
        <section key={item.title}>
          <h3>{item.title}</h3>
          <p>{item.body}</p>
          <code>{item.code}</code>
        </section>
      ))}
      <h2>{copy.capabilityLedgerTitle}</h2>
      <p>{copy.capabilityLedgerBody}</p>
      <p>{copy.capabilityReference}</p>
      {copy.capabilityGroups.map((group) => (
        <section key={group.id}>
          <h3>{group.title}</h3>
          <p>{group.summary}</p>
          <ul>
            {group.items.map((item) => (
              <li key={item.title}>
                <code>{item.signal}</code> <strong>{item.title}</strong>{' '}
                {item.body}
              </li>
            ))}
          </ul>
        </section>
      ))}
      <h2>{copy.workflowTitle}</h2>
      <p>{copy.workflowBody}</p>
      <h2>{copy.boundaryTitle}</h2>
      <p>{copy.boundaryBody}</p>
      <h2>{copy.surfacesTitle}</h2>
      <p>{copy.surfacesBody}</p>
    </main>
  );
}

export function HomeLayout() {
  const language = useLang();
  const locale: Locale = language === 'zh' ? 'zh' : 'en';
  const copy = homeCopy[locale];
  const [reviewStarted, setReviewStarted] = useState(false);
  const [openCapabilityGroups, setOpenCapabilityGroups] = useState(
    () => new Set(['context']),
  );
  const version = useVersion();
  const { site } = useSite();
  const defaultVersion = site.multiVersion.default;
  const installVersion =
    version === defaultVersion ? publishedVersion : version;
  const routePrefix = [
    version && version !== defaultVersion ? version : '',
    locale !== site.lang ? locale : '',
  ]
    .filter(Boolean)
    .join('/');
  const route = (pathname: string) => {
    const normalized = pathname.replace(/^\/+/, '');
    return withBase(`/${[routePrefix, normalized].filter(Boolean).join('/')}`);
  };
  const startExperience = () => {
    setReviewStarted(true);
    window.requestAnimationFrame(() => {
      document
        .getElementById('testkit-experience')
        ?.scrollIntoView({ behavior: 'smooth', block: 'center' });
    });
  };

  if (import.meta.env.SSG_MD) {
    return (
      <MarkdownHome
        installVersion={installVersion}
        locale={locale}
        version={version}
      />
    );
  }

  return (
    <main className="test-home">
      <section className="test-hero">
        <div className="test-hero-copy">
          <h1>
            <span className="test-hero-brand">A3S Test</span>
            {copy.heroTitle.map((line) => (
              <span key={line}>{line}</span>
            ))}
          </h1>
          <p>{copy.heroBody}</p>
          <div className="test-actions">
            <button
              className="test-button test-button-primary"
              onClick={startExperience}
              type="button"
            >
              {copy.startExperience}
              <ArrowRight aria-hidden="true" size={16} weight="bold" />
            </button>
            <a
              className="test-button test-button-secondary"
              href={route('/guide/')}
            >
              {copy.readDocs}
              <ArrowRight aria-hidden="true" size={16} weight="bold" />
            </a>
          </div>
        </div>
        <TestKitExperience
          copy={copy.experience}
          locale={locale}
          onStartReview={startExperience}
          reviewStarted={reviewStarted}
        />
      </section>

      <section className="test-installer-rail">
        <header>
          <h2>{copy.installTitle}</h2>
          <p>{copy.installBody}</p>
          <a
            className="test-installer-testkit-link"
            href={route('/guide/testkit.html')}
          >
            {copy.testkitInstallLink}
            <ArrowRight aria-hidden="true" size={14} weight="bold" />
          </a>
        </header>
        <div>
          <InstallSwitcher
            docsVersion={version}
            installVersion={installVersion}
            labels={copy}
          />
        </div>
      </section>

      <section className="test-section test-proof">
        <div className="test-section-copy">
          <h2>{copy.proofTitle}</h2>
          <p>{copy.proofBody}</p>
          <a href={route('/guide/contracts.html')}>
            {copy.contractGuide}{' '}
            <ArrowRight aria-hidden="true" size={15} weight="bold" />
          </a>
        </div>
        <ContractPanel labels={copy} />
      </section>

      <section className="test-section test-capabilities">
        <header>
          <h2>{copy.capabilitiesTitle}</h2>
          <p>{copy.capabilitiesBody}</p>
        </header>
        <div className="test-capability-grid">
          {copy.capabilities.map((item, index) => (
            <article
              className={`test-capability test-capability-${index + 1}`}
              key={item.title}
            >
              <code>{item.code}</code>
              <h3>{item.title}</h3>
              <p>{item.body}</p>
            </article>
          ))}
        </div>
        <div className="test-capability-ledger">
          <div className="test-capability-ledger-intro">
            <h3>{copy.capabilityLedgerTitle}</h3>
            <div className="test-capability-ledger-intro-copy">
              <p>{copy.capabilityLedgerBody}</p>
              <a
                href={route(
                  version === defaultVersion
                    ? '/reference/capabilities.html'
                    : '/reference/cli.html',
                )}
              >
                {copy.capabilityReference}{' '}
                <ArrowRight aria-hidden="true" size={15} weight="bold" />
              </a>
            </div>
          </div>
          <div className="test-capability-groups">
            {copy.capabilityGroups.map((group) => {
              const isOpen = openCapabilityGroups.has(group.id);
              const triggerId = `test-capability-trigger-${group.id}`;
              const panelId = `test-capability-panel-${group.id}`;
              return (
                <section
                  className={`test-capability-group${isOpen ? ' is-open' : ''}`}
                  key={group.id}
                >
                  <h4>
                    <button
                      aria-controls={panelId}
                      aria-expanded={isOpen}
                      data-testid={`capability-group-${group.id}`}
                      id={triggerId}
                      onClick={() =>
                        setOpenCapabilityGroups((current) => {
                          const next = new Set(current);
                          if (next.has(group.id)) next.delete(group.id);
                          else next.add(group.id);
                          return next;
                        })
                      }
                      type="button"
                    >
                      <code>{group.code}</code>
                      <span className="test-capability-group-title">
                        <strong>{group.title}</strong>
                        <small>{group.summary}</small>
                      </span>
                      <span className="test-capability-group-count">
                        {group.items.length} {copy.capabilityItemCount}
                      </span>
                      <CaretDown
                        aria-hidden="true"
                        className="test-capability-group-caret"
                        size={18}
                        weight="bold"
                      />
                    </button>
                  </h4>
                  <div
                    aria-labelledby={triggerId}
                    className="test-capability-group-body"
                    data-testid={`capability-panel-${group.id}`}
                    hidden={!isOpen}
                    id={panelId}
                    role="region"
                  >
                    <dl>
                      {group.items.map((item) => (
                        <div key={item.title}>
                          <dt>
                            <strong>{item.title}</strong>
                            <code>{item.signal}</code>
                          </dt>
                          <dd>{item.body}</dd>
                        </div>
                      ))}
                    </dl>
                    <a
                      href={route(
                        version === defaultVersion
                          ? group.href
                          : archivedCapabilityHrefs[group.id],
                      )}
                    >
                      {group.linkLabel}{' '}
                      <ArrowRight aria-hidden="true" size={15} weight="bold" />
                    </a>
                  </div>
                </section>
              );
            })}
          </div>
        </div>
      </section>

      <section className="test-section test-workflows">
        <header>
          <h2>{copy.workflowTitle}</h2>
          <p>{copy.workflowBody}</p>
        </header>
        <div className="test-workflow-track" aria-label={copy.workflowTitle}>
          <span>{copy.workflowObserve}</span>
          <span>{copy.workflowDecide}</span>
          <span>{copy.workflowAct}</span>
          <span>{copy.workflowProve}</span>
        </div>
        <div className="test-workflow-details">
          <article>
            <h3>{copy.workflowAgent}</h3>
            <p>{copy.workflowAgentBody}</p>
            <a href={route('/guide/workflows.html')}>
              {copy.workflowAgentLink}{' '}
              <ArrowRight aria-hidden="true" size={15} weight="bold" />
            </a>
          </article>
          <article>
            <h3>{copy.workflowAcl}</h3>
            <p>{copy.workflowAclBody}</p>
            <a href={route('/guide/workflows.html')}>
              {copy.workflowAclLink}{' '}
              <ArrowRight aria-hidden="true" size={15} weight="bold" />
            </a>
          </article>
        </div>
      </section>

      <section className="test-section test-boundary">
        <div className="test-boundary-copy">
          <h2>{copy.boundaryTitle}</h2>
          <p>{copy.boundaryBody}</p>
          <a href={route('/concepts/architecture.html')}>
            {copy.architecture}{' '}
            <ArrowRight aria-hidden="true" size={15} weight="bold" />
          </a>
        </div>
        <ol className="test-authority-layers">
          {[
            [copy.boundaryFacts, copy.boundaryFactsBody],
            [copy.boundaryAdvice, copy.boundaryAdviceBody],
            [copy.boundaryHuman, copy.boundaryHumanBody],
            [copy.boundaryRepair, copy.boundaryRepairBody],
          ].map(([title, body]) => (
            <li key={title}>
              <strong>{title}</strong>
              <span>{body}</span>
            </li>
          ))}
        </ol>
      </section>

      <section className="test-section test-surfaces">
        <header>
          <h2>{copy.surfacesTitle}</h2>
          <p>{copy.surfacesBody}</p>
        </header>
        <dl>
          <div className="is-primary">
            <dt>{copy.surfaceWeb}</dt>
            <dd>{copy.surfaceWebBody}</dd>
          </div>
          <div>
            <dt>{copy.surfaceGui}</dt>
            <dd>{copy.surfaceGuiBody}</dd>
          </div>
          <div>
            <dt>{copy.surfaceTui}</dt>
            <dd>{copy.surfaceTuiBody}</dd>
          </div>
        </dl>
      </section>

      <section className="test-cta">
        <div>
          <h2>{copy.ctaTitle}</h2>
          <p>{copy.ctaBody}</p>
        </div>
        <div className="test-actions">
          <a
            className="test-button test-button-primary"
            href={route('/guide/')}
          >
            {copy.quickStart}
            <ArrowRight aria-hidden="true" size={16} weight="bold" />
          </a>
          <a
            className="test-button test-button-secondary"
            href={route('/guide/testkit.html')}
          >
            {copy.testkitGuide}
          </a>
        </div>
      </section>

      <footer className="test-footer">
        <a href={route('/')}>A3S Test</a>
        <span>{copy.footer}</span>
        <a href="https://github.com/A3S-Lab/Test">
          GitHub <ArrowUpRight aria-hidden="true" size={14} weight="bold" />
        </a>
      </footer>
    </main>
  );
}
