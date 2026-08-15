import { useEffect, useMemo, useRef, useState } from 'react';
import type { Locale } from '../home-copy';

type CopyState = 'idle' | 'copied' | 'error';
type Platform = 'macos' | 'linux' | 'windows';

type Labels = {
  installTabs: string;
  installPackage: string;
  copy: string;
  copied: string;
  copyError: string;
};

const platformLabels: Record<Platform, string> = {
  macos: 'macOS',
  linux: 'Linux',
  windows: 'Windows',
};

const platforms = Object.keys(platformLabels) as Platform[];

function commandFor(platform: Platform, version: string, isCurrent: boolean) {
  if (platform === 'windows') {
    const command =
      "& ([scriptblock]::Create((irm 'https://github.com/A3S-Lab/Test/releases/latest/download/install.ps1')))";
    return isCurrent ? command : `${command} -Version ${version}`;
  }

  const command =
    'curl -fsSL https://github.com/A3S-Lab/Test/releases/latest/download/install.sh | sh';
  return isCurrent ? command : `${command} -s -- --version ${version}`;
}

export function InstallSwitcher({
  defaultVersion,
  labels,
  locale,
  version,
}: {
  defaultVersion: string;
  labels: Labels;
  locale: Locale;
  version: string;
}) {
  const [active, setActive] = useState<Platform>('macos');
  const [copyState, setCopyState] = useState<CopyState>('idle');
  const resetTimer = useRef<number | undefined>(undefined);
  const command = useMemo(
    () => commandFor(active, version, version === defaultVersion),
    [active, defaultVersion, version],
  );

  useEffect(
    () => () => {
      if (resetTimer.current !== undefined) {
        window.clearTimeout(resetTimer.current);
      }
    },
    [],
  );

  async function copyCommand() {
    try {
      await navigator.clipboard.writeText(command);
      setCopyState('copied');
    } catch {
      setCopyState('error');
    }

    if (resetTimer.current !== undefined) {
      window.clearTimeout(resetTimer.current);
    }
    resetTimer.current = window.setTimeout(() => setCopyState('idle'), 1800);
  }

  const copyLabel =
    copyState === 'copied'
      ? labels.copied
      : copyState === 'error'
        ? labels.copyError
        : labels.copy;

  return (
    <div className="test-install">
      <div
        aria-label={labels.installTabs}
        className="test-install-tabs"
        role="tablist"
      >
        {platforms.map((platform, index) => {
          const selected = platform === active;
          return (
            <button
              aria-controls="test-install-panel"
              aria-selected={selected}
              className={selected ? 'is-active' : undefined}
              id={`test-install-tab-${platform}`}
              key={platform}
              onClick={() => {
                setActive(platform);
                setCopyState('idle');
              }}
              onKeyDown={(event) => {
                let next = index;
                if (event.key === 'ArrowRight') next = index + 1;
                if (event.key === 'ArrowLeft') next = index - 1;
                if (event.key === 'Home') next = 0;
                if (event.key === 'End') next = platforms.length - 1;
                if (next === index) return;

                event.preventDefault();
                const target =
                  platforms[(next + platforms.length) % platforms.length];
                setActive(target);
                setCopyState('idle');
                window.requestAnimationFrame(() => {
                  document
                    .getElementById(`test-install-tab-${target}`)
                    ?.focus();
                });
              }}
              role="tab"
              tabIndex={selected ? 0 : -1}
              type="button"
            >
              {platformLabels[platform]}
            </button>
          );
        })}
      </div>
      <div
        aria-labelledby={`test-install-tab-${active}`}
        className="test-install-panel"
        id="test-install-panel"
        role="tabpanel"
      >
        <div className="test-install-meta">
          <span>{labels.installPackage}</span>
          <button
            aria-live="polite"
            className={copyState === 'copied' ? 'is-copied' : undefined}
            onClick={copyCommand}
            type="button"
          >
            {copyLabel}
          </button>
        </div>
        <div className="test-install-command" lang="en" tabIndex={0}>
          <span aria-hidden="true">{active === 'windows' ? 'PS›' : '$'}</span>
          <code>{command}</code>
        </div>
        <p className="test-install-note">
          {locale === 'zh'
            ? '安装器校验发布归档的 SHA-256，并保留版本一致性。'
            : 'The installer verifies the release archive SHA-256 and keeps versions aligned.'}
        </p>
      </div>
    </div>
  );
}
