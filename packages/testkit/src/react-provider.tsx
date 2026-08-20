import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type PropsWithChildren,
} from "react";
import { installTestKit } from "./runtime";
import { useBrowserLayoutEffect } from "./react-effect";
import { stableList } from "./review-utils";
import type {
  PageContextBridge,
  SourceSpan,
  TestKitOptions,
  TestKitRuntime,
} from "./types";

type TestKitContextValue = {
  bridge: PageContextBridge | null;
  providerConfigured: boolean;
};

const TestKitContext = createContext<TestKitContextValue>({
  bridge: null,
  providerConfigured: false,
});

export function useTestKitContext(): TestKitContextValue {
  return useContext(TestKitContext);
}

export type A3STestKitProps = PropsWithChildren<
  Omit<TestKitOptions, "enabled"> & { enabled: boolean }
>;

export function A3STestKit({ children, ...options }: A3STestKitProps) {
  const latest = useLatest(options);
  const [bridge, setBridge] = useState<PageContextBridge | null>(null);

  useEffect(() => {
    if (options.enabled !== true) {
      setBridge(null);
      return;
    }
    const installed = installTestKit({
      ...options,
      enabled: true,
      ready: () =>
        latest.current.ready?.() ?? document.readyState !== "loading",
      facts: () => latest.current.facts?.() ?? {},
    });
    setBridge(installed);
    return () => {
      installed.dispose();
      setBridge((current) => (current === installed ? null : current));
    };
  }, [
    options.enabled,
    options.maxDesignAuditReports,
    options.maxEncodedBytes,
    options.maxNodes,
    options.maxQualityReports,
    options.maxStringBytes,
    options.maxUiDurationMs,
    options.maxUiEncodedBytes,
    options.maxUiNodes,
    options.maxUiStateSamples,
    options.page.id,
    options.repairEndpoint,
    options.repairStorage,
    options.uiUnderstanding,
    stableList(options.redact),
  ]);

  const value = useMemo(() => ({ bridge, providerConfigured: true }), [bridge]);
  return (
    <TestKitContext.Provider value={value}>{children}</TestKitContext.Provider>
  );
}

export type A3STestBoundaryProps = PropsWithChildren<{
  id: string;
  name: string;
  source?: SourceSpan;
  generated?: SourceSpan;
  ready?: () => boolean;
  facts?: () => Record<string, unknown>;
  roots?: () => readonly Element[];
  as?:
    | "div"
    | "section"
    | "main"
    | "nav"
    | "article"
    | "aside"
    | "header"
    | "footer"
    | "span";
  className?: string;
  style?: CSSProperties;
}>;

export function A3STestBoundary({
  id,
  name,
  source,
  generated,
  ready,
  facts,
  roots,
  as: Tag = "div",
  children,
  className,
  style,
}: A3STestBoundaryProps) {
  const { bridge } = useTestKitContext();
  const ref = useRef<HTMLElement | null>(null);
  const latest = useLatest({ ready, facts, roots });
  useBrowserLayoutEffect(() => {
    const element = ref.current;
    if (!element || !bridge || !("registerBoundary" in bridge)) return;
    return (bridge as TestKitRuntime).registerBoundary({
      id,
      name,
      elements: () => [element, ...(latest.current.roots?.() ?? [])],
      ...(source ? { source } : {}),
      ...(generated ? { generated } : {}),
      ready: () => latest.current.ready?.() ?? true,
      facts: () => latest.current.facts?.() ?? {},
    });
  }, [
    bridge,
    generated?.column,
    generated?.endColumn,
    generated?.endLine,
    generated?.file,
    generated?.line,
    id,
    name,
    source?.column,
    source?.endColumn,
    source?.endLine,
    source?.file,
    source?.line,
  ]);
  return (
    <Tag ref={ref as never} className={className} style={style}>
      {children}
    </Tag>
  );
}

function useLatest<T>(value: T) {
  const ref = useRef(value);
  ref.current = value;
  return ref;
}
