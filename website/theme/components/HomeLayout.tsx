import { useLang, useSite, useVersion, withBase } from '@rspress/core/runtime';
import { EvidencePanel } from './EvidencePanel';
import { InstallSwitcher, installCommandFor } from './InstallSwitcher';
import { homeCopy, type Locale } from '../home-copy';

function MarkdownHome({
  defaultVersion,
  locale,
  version,
}: {
  defaultVersion: string;
  locale: Locale;
  version: string;
}) {
  const copy = homeCopy[locale];
  const unixInstall = installCommandFor('macos', version, defaultVersion);
  const windowsInstall = installCommandFor('windows', version, defaultVersion);

  return (
    <main>
      <h1>{copy.heroTitle.join(locale === 'zh' ? '' : ' ')}</h1>
      <p>{copy.heroBody}</p>
      <h2>{copy.installTitle}</h2>
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
      <h2>{copy.capabilitiesTitle}</h2>
      {copy.capabilities.map((item) => (
        <section key={item.title}>
          <h3>{item.title}</h3>
          <p>{item.body}</p>
          <code>{item.code}</code>
        </section>
      ))}
      <h2>{copy.boundaryTitle}</h2>
      <p>{copy.boundaryBody}</p>
      <h2>{copy.surfacesTitle}</h2>
      <p>{copy.surfacesBody}</p>
    </main>
  );
}

function Arrow() {
  return <span aria-hidden="true">→</span>;
}

export function HomeLayout() {
  const language = useLang();
  const locale: Locale = language === 'zh' ? 'zh' : 'en';
  const copy = homeCopy[locale];
  const version = useVersion();
  const { site } = useSite();
  const defaultVersion = site.multiVersion.default;
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

  if (import.meta.env.SSG_MD) {
    return (
      <MarkdownHome
        defaultVersion={defaultVersion}
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
            {copy.heroTitle.map((line) => (
              <span key={line}>{line}</span>
            ))}
          </h1>
          <p>{copy.heroBody}</p>
          <div className="test-actions">
            <a
              className="test-button test-button-primary"
              href={route('/guide/')}
            >
              {copy.readDocs}
              <Arrow />
            </a>
            <a
              className="test-button test-button-secondary"
              href="https://github.com/A3S-Lab/Test"
            >
              {copy.viewGitHub}
            </a>
          </div>
        </div>
        <div className="test-hero-install">
          <header>
            <h2>{copy.installTitle}</h2>
            <p>{copy.installBody}</p>
          </header>
          <InstallSwitcher
            defaultVersion={defaultVersion}
            labels={copy}
            version={version}
          />
        </div>
      </section>

      <section className="test-section test-proof">
        <div className="test-section-copy">
          <h2>{copy.proofTitle}</h2>
          <p>{copy.proofBody}</p>
        </div>
        <EvidencePanel labels={copy} locale={locale} />
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
              {copy.readDocs} <Arrow />
            </a>
          </article>
          <article>
            <h3>{copy.workflowAcl}</h3>
            <p>{copy.workflowAclBody}</p>
            <a href={route('/guide/workflows.html')}>
              ACL <Arrow />
            </a>
          </article>
        </div>
      </section>

      <section className="test-section test-boundary">
        <div className="test-boundary-copy">
          <h2>{copy.boundaryTitle}</h2>
          <p>{copy.boundaryBody}</p>
          <a href={route('/concepts/architecture.html')}>
            {copy.architecture} <Arrow />
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
            <Arrow />
          </a>
          <a
            className="test-button test-button-secondary"
            href={route('/concepts/architecture.html')}
          >
            {copy.architecture}
          </a>
        </div>
      </section>

      <footer className="test-footer">
        <a href={route('/')}>A3S Test</a>
        <span>{copy.footer}</span>
        <a href="https://github.com/A3S-Lab/Test">GitHub ↗</a>
      </footer>
    </main>
  );
}
