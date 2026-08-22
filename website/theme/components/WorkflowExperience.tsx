import {
  ArrowClockwise,
  Camera,
  CheckCircle,
  Code,
  CursorClick,
  Eye,
  FileText,
  Package,
  Pause,
  Play,
  Scan,
  ShieldCheck,
  WarningCircle,
} from '@phosphor-icons/react';
import { useEffect, useRef, useState } from 'react';
import type { WorkflowCopy } from '../home-copy-types';

const phaseIcons = [FileText, Scan, Code, CursorClick, Package, CheckCircle];

const sourceIcons = [FileText, Scan, CursorClick];
const evidenceIcons = [Camera, Eye, Code, WarningCircle, Package];

export function WorkflowExperience({ copy }: { copy: WorkflowCopy }) {
  const [activeStep, setActiveStep] = useState(0);
  const [inView, setInView] = useState(false);
  const [pageVisible, setPageVisible] = useState(true);
  const [paused, setPaused] = useState(false);
  const [manualOnly, setManualOnly] = useState(false);
  const shellRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const query = window.matchMedia(
      '(prefers-reduced-motion: reduce), (max-width: 768px)',
    );
    const update = () => setManualOnly(query.matches);
    update();
    query.addEventListener('change', update);
    return () => query.removeEventListener('change', update);
  }, []);

  useEffect(() => {
    const update = () => setPageVisible(!document.hidden);
    update();
    document.addEventListener('visibilitychange', update);
    return () => document.removeEventListener('visibilitychange', update);
  }, []);

  useEffect(() => {
    const shell = shellRef.current;
    if (!shell) return;
    if (typeof IntersectionObserver === 'undefined') {
      setInView(true);
      return;
    }
    const observer = new IntersectionObserver(
      ([entry]) => setInView(Boolean(entry?.isIntersecting)),
      { threshold: 0.24 },
    );
    observer.observe(shell);
    return () => observer.disconnect();
  }, []);

  const playing = inView && pageVisible && !paused && !manualOnly;

  useEffect(() => {
    if (!playing) return;
    const timer = window.setTimeout(() => {
      setActiveStep((current) => (current + 1) % copy.steps.length);
    }, 5200);
    return () => window.clearTimeout(timer);
  }, [activeStep, copy.steps.length, playing]);

  const active = copy.steps[activeStep];
  const ActiveIcon = phaseIcons[activeStep];

  return (
    <section
      aria-labelledby="test-workflow-title"
      className="test-section test-workflow"
      id="workflow"
    >
      <header className="test-workflow-heading">
        <div>
          <h2 id="test-workflow-title">{copy.title}</h2>
          <p>{copy.body}</p>
        </div>
        <aside>
          <ShieldCheck aria-hidden="true" size={20} weight="fill" />
          <div>
            <strong>{copy.boundaryTitle}</strong>
            <p>{copy.boundaryBody}</p>
          </div>
        </aside>
      </header>

      <div
        aria-label={copy.stageAria}
        className={`test-workflow-shell${playing ? ' is-playing' : ''}`}
        ref={shellRef}
      >
        <div className="test-workflow-bar">
          <span>
            <i aria-hidden="true" />
            {copy.stageLabel}
          </span>
          <code>a3s.test.workflow/1</code>
          <button
            aria-label={
              manualOnly ? copy.manual : paused ? copy.resume : copy.pause
            }
            aria-pressed={manualOnly || paused}
            disabled={manualOnly}
            onClick={() => setPaused((current) => !current)}
            title={manualOnly ? copy.manual : paused ? copy.resume : copy.pause}
            type="button"
          >
            {paused || manualOnly ? (
              <Play aria-hidden="true" size={13} weight="fill" />
            ) : (
              <Pause aria-hidden="true" size={13} weight="fill" />
            )}
            <span>
              {manualOnly ? copy.manual : paused ? copy.resume : copy.pause}
            </span>
          </button>
          <small className="test-workflow-manual">{copy.manual}</small>
        </div>

        <ol className="test-workflow-steps" aria-label={copy.stageAria}>
          {copy.steps.map((step, index) => {
            const Icon = phaseIcons[index];
            const isActive = index === activeStep;
            return (
              <li key={step.label}>
                <button
                  aria-current={isActive ? 'step' : undefined}
                  aria-label={step.label}
                  className={isActive ? 'is-active' : undefined}
                  data-testid={`workflow-step-${index + 1}`}
                  onClick={() => {
                    setActiveStep(index);
                    setPaused(true);
                  }}
                  type="button"
                >
                  <span>{String(index + 1).padStart(2, '0')}</span>
                  <Icon aria-hidden="true" size={17} weight="bold" />
                  <strong>{step.label}</strong>
                  <i aria-hidden="true" />
                </button>
              </li>
            );
          })}
        </ol>

        <article
          aria-atomic="true"
          aria-live="polite"
          className="test-workflow-active"
          key={activeStep}
        >
          <header>
            <div>
              <div className="test-workflow-active-title">
                <ActiveIcon aria-hidden="true" size={22} weight="bold" />
                <h3>{active.title}</h3>
              </div>
              <p>{active.body}</p>
            </div>
            <span>{active.status}</span>
          </header>

          <div className="test-workflow-path">
            <section className="test-workflow-source">
              <h4>{active.sourceTitle}</h4>
              <ul>
                {active.sources.map((source, index) => {
                  const Icon = sourceIcons[index];
                  return (
                    <li key={source.label}>
                      <Icon aria-hidden="true" size={19} weight="bold" />
                      <span>
                        <strong>{source.label}</strong>
                        <small>{source.detail}</small>
                      </span>
                    </li>
                  );
                })}
              </ul>
            </section>

            <section className="test-workflow-process">
              <header>
                <Scan aria-hidden="true" size={16} weight="bold" />
                <span>{active.processTitle}</span>
              </header>
              <h4>{active.processLead}</h4>
              <dl>
                {active.processRows.map((row) => (
                  <div key={row.label}>
                    <dt>{row.label}</dt>
                    <dd>{row.value}</dd>
                  </div>
                ))}
              </dl>
              <div className="test-workflow-pulse" aria-hidden="true">
                <i />
                <i />
                <i />
              </div>
            </section>

            <section className="test-workflow-output">
              <header>
                <Code aria-hidden="true" size={16} weight="bold" />
                <h4>{active.outputTitle}</h4>
                <span>
                  <i aria-hidden="true" /> READY
                </span>
              </header>
              <pre>
                <code>
                  {active.outputLines.map((line) => {
                    const [key, ...rest] = line.trim().split(/\s+/);
                    return (
                      <span key={line}>
                        <b>{key}</b>
                        {rest.join(' ')}
                      </span>
                    );
                  })}
                </code>
              </pre>
            </section>

            <div className="test-workflow-route" aria-hidden="true">
              <i />
              <i />
            </div>
          </div>

          <footer className="test-workflow-evidence">
            <h4>{copy.evidenceTitle}</h4>
            <div>
              {copy.evidenceItems.map((item, index) => {
                const Icon = evidenceIcons[index];
                return (
                  <span key={item.label}>
                    <Icon aria-hidden="true" size={17} weight="bold" />
                    <b>{item.label}</b>
                    <small>{item.detail}</small>
                  </span>
                );
              })}
            </div>
            <p>
              <ArrowClockwise aria-hidden="true" size={15} weight="bold" />
              {copy.evidenceNote}
            </p>
          </footer>
        </article>
      </div>
    </section>
  );
}
