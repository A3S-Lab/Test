import { useLang, useSite, useVersion, withBase } from '@rspress/core/runtime';
import {
  ArrowRight,
  ArrowUpRight,
  CaretDown,
  ChartBar,
  CheckCircle,
  Code,
  CursorClick,
  FileText,
  ShieldCheck,
  Timer,
} from '@phosphor-icons/react';
import { useState } from 'react';
import { InstallSwitcher, installCommandFor } from './InstallSwitcher';
import { TestKitExperience } from './TestKitExperience';
import { homeCopy, type Locale } from '../home-copy';
import type { BenchmarkCopy } from '../home-copy-types';
import { publishedVersion } from '../../versions.mjs';
import benchmarkSummary from '../../../benchmarks/ui/results/summary/20260821T171716Z.json';

const benchmarkReportUrl =
  'https://github.com/A3S-Lab/Test/tree/main/benchmarks/ui';

const benchmarkCandidate = benchmarkSummary.candidates['a3s-test'];
const benchmarkBaseline = benchmarkSummary.candidates['agent-browser'];
const benchmarkPairs = benchmarkSummary.paired_common_success;

type BenchmarkRow = {
  label: string;
  candidateValue: string;
  candidateDetail: string;
  baselineValue: string;
  baselineDetail: string;
};

function formatPercent(value: number) {
  const percentage = value * 100;
  return `${percentage.toFixed(Number.isInteger(percentage) ? 0 : 1)}%`;
}

function formatDuration(milliseconds: number, locale: Locale) {
  return `${(milliseconds / 1000).toFixed(2)} ${locale === 'zh' ? '秒' : 's'}`;
}

function benchmarkScope(copy: BenchmarkCopy) {
  const { protocol, host } = benchmarkSummary;
  return `${copy.labels.lockedProtocol} · ${protocol.task_count} ${copy.labels.tasks} × ${protocol.repetitions} ${copy.labels.repetitions} · ${host.cpu_model} · ${protocol.viewport.width} × ${protocol.viewport.height}`;
}

function benchmarkRows(copy: BenchmarkCopy, locale: Locale): BenchmarkRow[] {
  const candidateStale = benchmarkCandidate.probes.stale_ref;
  const baselineStale = benchmarkBaseline.probes.stale_ref;
  const candidateRejected = Math.round(
    candidateStale.rejection_rate * candidateStale.runs,
  );
  const baselineRejected = Math.round(
    baselineStale.rejection_rate * baselineStale.runs,
  );
  const candidateMutations = Math.round(
    candidateStale.page_mutation_rate * candidateStale.runs,
  );
  const baselineMutations = Math.round(
    baselineStale.page_mutation_rate * baselineStale.runs,
  );
  const candidateArtifactRuns = Math.round(
    benchmarkCandidate.evidence_file_rate * benchmarkCandidate.runs,
  );
  const baselineArtifactRuns = Math.round(
    benchmarkBaseline.evidence_file_rate * benchmarkBaseline.runs,
  );

  return [
    {
      label: copy.metrics.success,
      candidateValue: formatPercent(benchmarkCandidate.success_rate),
      candidateDetail: `${benchmarkCandidate.passed} / ${benchmarkCandidate.runs} ${copy.labels.mainRuns}`,
      baselineValue: formatPercent(benchmarkBaseline.success_rate),
      baselineDetail: `${benchmarkBaseline.passed} / ${benchmarkBaseline.runs} ${copy.labels.mainRuns}`,
    },
    {
      label: copy.metrics.staleReference,
      candidateValue: `${candidateRejected} / ${candidateStale.runs}`,
      candidateDetail: `${copy.labels.staleRejected} · ${candidateMutations} ${copy.labels.pageMutations}`,
      baselineValue: `${baselineRejected} / ${baselineStale.runs}`,
      baselineDetail: `${copy.labels.staleRejected} · ${baselineMutations} ${copy.labels.pageMutations}`,
    },
    {
      label: copy.metrics.evidence,
      candidateValue: `${candidateArtifactRuns} / ${benchmarkCandidate.runs}`,
      candidateDetail: copy.labels.artifactRuns,
      baselineValue: `${baselineArtifactRuns} / ${benchmarkBaseline.runs}`,
      baselineDetail: copy.labels.artifactRuns,
    },
    {
      label: copy.metrics.latency,
      candidateValue: formatDuration(
        benchmarkPairs.candidate_execution_ms.median,
        locale,
      ),
      candidateDetail: `+${formatPercent(benchmarkPairs.overhead_ratio.median)} ${copy.labels.versusDirect}`,
      baselineValue: formatDuration(
        benchmarkPairs.baseline_execution_ms.median,
        locale,
      ),
      baselineDetail: copy.labels.hostBaseline,
    },
  ];
}

const benchmarkIcons = [CheckCircle, ShieldCheck, FileText, Timer];

function BenchmarkSection({
  copy,
  locale,
}: {
  copy: BenchmarkCopy;
  locale: Locale;
}) {
  const rows = benchmarkRows(copy, locale);

  return (
    <section
      aria-labelledby="test-benchmark-title"
      className="test-section test-benchmark"
    >
      <header>
        <h2 id="test-benchmark-title">{copy.title}</h2>
        <p>{copy.body}</p>
      </header>

      <div className="test-benchmark-sheet">
        <div className="test-benchmark-sheet-meta">
          <span>
            <ChartBar aria-hidden="true" size={17} weight="bold" />
            {benchmarkScope(copy)}
          </span>
          <code>run {benchmarkSummary.run_id}</code>
        </div>

        <div className="test-benchmark-table">
          <table>
            <caption>{copy.tableCaption}</caption>
            <thead>
              <tr>
                <th scope="col">{copy.dimension}</th>
                <th scope="col">
                  <span className="test-benchmark-candidate is-a3s">
                    <i aria-hidden="true" />
                    {copy.candidate}
                  </span>
                </th>
                <th scope="col">
                  <span className="test-benchmark-candidate">
                    <i aria-hidden="true" />
                    {copy.baseline}
                  </span>
                </th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row, index) => {
                const Icon = benchmarkIcons[index];
                return (
                  <tr key={row.label}>
                    <th scope="row">
                      <span className="test-benchmark-metric">
                        <Icon aria-hidden="true" size={18} weight="bold" />
                        {row.label}
                      </span>
                    </th>
                    <td data-label={copy.candidate}>
                      <strong>{row.candidateValue}</strong>
                      <span>{row.candidateDetail}</span>
                    </td>
                    <td data-label={copy.baseline}>
                      <strong>{row.baselineValue}</strong>
                      <span>{row.baselineDetail}</span>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>

        <div className="test-benchmark-footnote">
          <p>{copy.limitation}</p>
          <a href={benchmarkReportUrl}>
            {copy.reportLink}
            <ArrowUpRight aria-hidden="true" size={15} weight="bold" />
          </a>
        </div>
      </div>
    </section>
  );
}

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
  const rows = benchmarkRows(copy.benchmark, locale);

  return (
    <main>
      <h1>{copy.heroTitle.join(locale === 'zh' ? '' : ' ')}</h1>
      <p>{copy.heroBody}</p>
      {copy.proofItems.map((item) => (
        <section key={item.title}>
          <h2>{item.title}</h2>
          <p>{item.body}</p>
        </section>
      ))}
      <h2>{copy.installTitle}</h2>
      <p>{copy.installBody}</p>
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
      <h2>{copy.benchmark.title}</h2>
      <p>{copy.benchmark.body}</p>
      <table>
        <caption>{copy.benchmark.tableCaption}</caption>
        <thead>
          <tr>
            <th>{copy.benchmark.dimension}</th>
            <th>{copy.benchmark.candidate}</th>
            <th>{copy.benchmark.baseline}</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row.label}>
              <th>{row.label}</th>
              <td>{`${row.candidateValue} · ${row.candidateDetail}`}</td>
              <td>{`${row.baselineValue} · ${row.baselineDetail}`}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <p>
        <code>{`run ${benchmarkSummary.run_id}`}</code>
      </p>
      <p>{benchmarkScope(copy.benchmark)}</p>
      <p>{copy.benchmark.limitation}</p>
      <p>
        <a href={benchmarkReportUrl}>{copy.benchmark.reportLink}</a>
      </p>
      <h2>{copy.packetTitle}</h2>
      <p>{copy.packetBody}</p>
      <pre>
        <code>{copy.packetLines.join('\n')}</code>
      </pre>
      <p>{copy.packetTrust}</p>
      <h2>{copy.quickStartTitle}</h2>
      <p>{copy.quickStartBody}</p>
      {copy.quickStartSteps.map((step) => (
        <section key={step.title}>
          <h3>{step.title}</h3>
          <p>{step.body}</p>
          <pre>
            <code>{step.command}</code>
          </pre>
        </section>
      ))}
      <h2>{copy.faqTitle}</h2>
      <p>{copy.faqBody}</p>
      {copy.faqItems.map((item) => (
        <section key={item.question}>
          <h3>{item.question}</h3>
          <p>{item.answer}</p>
        </section>
      ))}
    </main>
  );
}

const proofIcons = [CursorClick, Code, CheckCircle];

export function HomeLayout() {
  const language = useLang();
  const locale: Locale = language === 'zh' ? 'zh' : 'en';
  const copy = homeCopy[locale];
  const [reviewStarted, setReviewStarted] = useState(false);
  const [openFaqs, setOpenFaqs] = useState(() => new Set<number>());
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

        <div className="test-proof-strip">
          {copy.proofItems.map((item, index) => {
            const Icon = proofIcons[index];
            return (
              <article key={item.title}>
                <Icon aria-hidden="true" size={18} weight="bold" />
                <div>
                  <h2>{item.title}</h2>
                  <p>{item.body}</p>
                </div>
              </article>
            );
          })}
        </div>
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
        <InstallSwitcher
          docsVersion={version}
          installVersion={installVersion}
          labels={copy}
        />
      </section>

      <BenchmarkSection copy={copy.benchmark} locale={locale} />

      <section className="test-section test-packet">
        <div className="test-packet-copy">
          <h2>{copy.packetTitle}</h2>
          <p>{copy.packetBody}</p>
          <a
            href={route(
              version === defaultVersion
                ? '/concepts/authority-and-safety.html'
                : '/concepts/architecture.html',
            )}
          >
            {copy.packetLink}
            <ArrowRight aria-hidden="true" size={15} weight="bold" />
          </a>
        </div>
        <div className="test-packet-code">
          <header>
            <Code aria-hidden="true" size={17} weight="bold" />
            <span>{copy.packetLabel}</span>
          </header>
          <pre>
            <code>{copy.packetLines.join('\n')}</code>
          </pre>
          <p>
            <ShieldCheck aria-hidden="true" size={17} weight="fill" />
            {copy.packetTrust}
          </p>
        </div>
      </section>

      <section className="test-section test-quickstart">
        <header>
          <h2>{copy.quickStartTitle}</h2>
          <p>{copy.quickStartBody}</p>
        </header>
        <ol className="test-quickstart-steps">
          {copy.quickStartSteps.map((step, index) => (
            <li key={step.title}>
              <span aria-hidden="true">{index + 1}</span>
              <div>
                <h3>{step.title}</h3>
                <p>{step.body}</p>
              </div>
              <pre>
                <code>{step.command}</code>
              </pre>
            </li>
          ))}
        </ol>
        <a className="test-inline-link" href={route('/guide/')}>
          {copy.quickStartLink}
          <ArrowRight aria-hidden="true" size={15} weight="bold" />
        </a>
      </section>

      <section className="test-section test-faq">
        <header>
          <h2>{copy.faqTitle}</h2>
          <p>{copy.faqBody}</p>
        </header>
        <div className="test-faq-list">
          {copy.faqItems.map((item, index) => {
            const isOpen = openFaqs.has(index);
            const triggerId = `test-faq-trigger-${index}`;
            const panelId = `test-faq-panel-${index}`;
            return (
              <section className="test-faq-item" key={item.question}>
                <h3>
                  <button
                    aria-controls={panelId}
                    aria-expanded={isOpen}
                    id={triggerId}
                    onClick={() =>
                      setOpenFaqs((current) => {
                        const next = new Set(current);
                        if (next.has(index)) next.delete(index);
                        else next.add(index);
                        return next;
                      })
                    }
                    type="button"
                  >
                    <span>{item.question}</span>
                    <CaretDown aria-hidden="true" size={18} weight="bold" />
                  </button>
                </h3>
                <div
                  aria-labelledby={triggerId}
                  hidden={!isOpen}
                  id={panelId}
                  role="region"
                >
                  <p>{item.answer}</p>
                </div>
              </section>
            );
          })}
        </div>
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
