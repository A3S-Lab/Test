export type Locale = 'zh' | 'en';

type LocalizedCopy = {
  heroTitle: [string, string];
  heroBody: string;
  readDocs: string;
  viewGitHub: string;
  installTitle: string;
  installBody: string;
  installTabs: string;
  installPackage: string;
  installNote: string;
  copy: string;
  copied: string;
  copyError: string;
  proofTitle: string;
  proofBody: string;
  sampleLabel: string;
  observationLabel: string;
  revisionLabel: string;
  actionLabel: string;
  evidenceLabel: string;
  pageContextLabel: string;
  pageContextValue: string;
  actionValue: string;
  evidenceValue: string;
  capabilitiesTitle: string;
  capabilitiesBody: string;
  capabilities: Array<{ title: string; body: string; code: string }>;
  workflowTitle: string;
  workflowBody: string;
  workflowAgent: string;
  workflowAgentBody: string;
  workflowAcl: string;
  workflowAclBody: string;
  workflowObserve: string;
  workflowDecide: string;
  workflowAct: string;
  workflowProve: string;
  boundaryTitle: string;
  boundaryBody: string;
  boundaryFacts: string;
  boundaryFactsBody: string;
  boundaryAdvice: string;
  boundaryAdviceBody: string;
  boundaryHuman: string;
  boundaryHumanBody: string;
  boundaryRepair: string;
  boundaryRepairBody: string;
  surfacesTitle: string;
  surfacesBody: string;
  surfaceWeb: string;
  surfaceWebBody: string;
  surfaceGui: string;
  surfaceGuiBody: string;
  surfaceTui: string;
  surfaceTuiBody: string;
  ctaTitle: string;
  ctaBody: string;
  quickStart: string;
  architecture: string;
  footer: string;
};

export const homeCopy: Record<Locale, LocalizedCopy> = {
  zh: {
    heroTitle: ['看懂界面，', '证明每次操作'],
    heroBody:
      '探索未知路径，验证真实结果，再把稳定流程固化为类型化 ACL 回归测试。',
    readDocs: '开始使用',
    viewGitHub: '查看 GitHub',
    installTitle: '一条命令开始测试',
    installBody: '安装 CLI，并为已检测到的编码 Agent 安装同版本 Skill。',
    installTabs: '选择安装平台',
    installPackage: 'CLI + Agent Skill',
    installNote: '安装器校验发布归档的 SHA-256，并保持 CLI 与 Skill 版本一致。',
    copy: '复制命令',
    copied: '已复制',
    copyError: '复制失败',
    proofTitle: '每一次操作都有来源和证据',
    proofBody:
      '页面结构、语义引用、坐标和组件归属进入同一份版本化上下文。动作必须绑定最新观察，结果写入可检查报告。',
    sampleLabel: '页面上下文示例',
    observationLabel: '观察',
    revisionLabel: '界面修订',
    actionLabel: '类型化动作',
    evidenceLabel: '持久证据',
    pageContextLabel: '当前目标',
    pageContextValue: '@c7 button “Place order”',
    actionValue: 'click @c7 · observation 42',
    evidenceValue: 'events.jsonl + report.json',
    capabilitiesTitle: '从发现问题到关闭回归环路',
    capabilitiesBody:
      '同一套执行边界服务于 Agent 探索、ACL 套件、嵌入式页面上下文和人工修复授权。',
    capabilities: [
      {
        title: 'Agent 按最新界面状态决策',
        body: '观察一次，执行一个通过 Schema 与策略校验的动作，再重新观察。',
        code: 'agent observe → agent click',
      },
      {
        title: '稳定路径变成 ACL 回归',
        body: '关闭式清单在运行前完成准入校验，断言、证据和退出码保持稳定。',
        code: 'check → run → report',
      },
      {
        title: 'Test Kit 在渲染后发布上下文',
        body: '可访问语义、组件标识、定位器、样式和多坐标空间几何按修订号读取。',
        code: '@a3s-lab/testkit',
      },
      {
        title: '人决定是否进入修复',
        body: '单选或批量标记先进入账本，只有明确授权才交给拥有工作区的编码 Agent。',
        code: 'review → authorize → verify',
      },
    ],
    workflowTitle: '一个内核，两种主要工作方式',
    workflowBody:
      '未知路径由调用方 Agent 规划，已知路径由 ACL 清单规划。二者共享动作、驱动、证据和清理责任。',
    workflowAgent: '探索性 Agent 会话',
    workflowAgentBody: '适合复现问题、检查未知流程和进行人工协作式界面评审。',
    workflowAcl: '确定性 ACL 套件',
    workflowAclBody: '适合稳定回归、CI、分布式执行和可重复的界面契约验证。',
    workflowObserve: '观察',
    workflowDecide: '决策',
    workflowAct: '动作',
    workflowProve: '证明',
    boundaryTitle: '模型能建议，不能替你授权',
    boundaryBody:
      '确定性事实、模型建议、人工决定和工作区修改分属四层权限，不能互相冒充。',
    boundaryFacts: '事实',
    boundaryFactsBody: '浏览器观察、Test Kit 上下文、文件摘要和本地断言。',
    boundaryAdvice: '建议',
    boundaryAdviceBody:
      '界面定位、设计审计和源到契约候选保持可追溯且默认非阻断。',
    boundaryHuman: '授权',
    boundaryHumanBody: '人工确认目标、批次、冲突决策和修复意图。',
    boundaryRepair: '执行',
    boundaryRepairBody: '拥有会话的编码 Agent 修改源码，并由新浏览器运行验证。',
    surfacesTitle: '跨界面执行，保持同一结果契约',
    surfacesBody: 'Web、GUI 与 TUI 适配器实现各自运行时，核心不选择具体后端。',
    surfaceWeb: 'Web',
    surfaceWebBody:
      '持久 Agent 会话与 ACL 套件，使用 A3S Browser 或兼容的独立浏览器。',
    surfaceGui: 'GUI',
    surfaceGuiBody:
      'macOS 上通过锁定的 A3S CUA 契约测试并完成真实主机发布认证。',
    surfaceTui: 'TUI',
    surfaceTuiBody: '通过自有 PTY / ConPTY 进程树和有界终端语义运行 ACL 套件。',
    ctaTitle: '先跑通一个真实流程',
    ctaBody: '从快速开始进入持久 Web 会话，再把已验证路径保存为 ACL。',
    quickStart: '阅读快速开始',
    architecture: '查看架构边界',
    footer: '类型化动作，可检查证据，自有清理责任。',
  },
  en: {
    heroTitle: ['See interfaces.', 'Prove actions.'],
    heroBody:
      'Explore unknown paths, verify real outcomes, then preserve stable workflows as typed ACL regression suites.',
    readDocs: 'Get started',
    viewGitHub: 'View GitHub',
    installTitle: 'Start testing with one command',
    installBody:
      'Install the CLI and the matching Skill for detected coding agents.',
    installTabs: 'Choose an installation platform',
    installPackage: 'CLI + Agent Skill',
    installNote:
      'The installer verifies the release archive SHA-256 and keeps the CLI and Skill on the same version.',
    copy: 'Copy command',
    copied: 'Copied',
    copyError: 'Copy failed',
    proofTitle: 'Every action carries provenance and evidence',
    proofBody:
      'Structure, semantic refs, geometry, and component ownership share one revisioned context. Actions bind to fresh observations and results remain inspectable.',
    sampleLabel: 'Sample page context',
    observationLabel: 'Observation',
    revisionLabel: 'Surface revision',
    actionLabel: 'Typed action',
    evidenceLabel: 'Durable evidence',
    pageContextLabel: 'Current target',
    pageContextValue: '@c7 button “Place order”',
    actionValue: 'click @c7 · observation 42',
    evidenceValue: 'events.jsonl + report.json',
    capabilitiesTitle: 'Close the loop from discovery to regression',
    capabilitiesBody:
      'One execution boundary serves agent exploration, ACL suites, embedded page context, and human-authorized repair.',
    capabilities: [
      {
        title: 'Agents decide from fresh interface state',
        body: 'Observe once, execute one schema- and policy-checked action, then observe again.',
        code: 'agent observe → agent click',
      },
      {
        title: 'Stable paths become ACL regressions',
        body: 'Closed manifests pass admission before launch and keep assertions, evidence, and exit codes stable.',
        code: 'check → run → report',
      },
      {
        title: 'Test Kit publishes rendered context',
        body: 'Accessible semantics, component identity, locators, styles, and multi-space geometry are revisioned.',
        code: '@a3s-lab/testkit',
      },
      {
        title: 'People decide what reaches repair',
        body: 'Single or batch marks enter a ledger and reach the workspace-owning agent only after authorization.',
        code: 'review → authorize → verify',
      },
    ],
    workflowTitle: 'One core, two primary workflows',
    workflowBody:
      'The calling agent plans unknown paths. ACL manifests plan known paths. Both share actions, drivers, evidence, and cleanup ownership.',
    workflowAgent: 'Exploratory agent session',
    workflowAgentBody:
      'Best for reproducing bugs, checking unknown flows, and collaborative interface review.',
    workflowAcl: 'Deterministic ACL suite',
    workflowAclBody:
      'Best for stable regression, CI, distributed execution, and repeatable surface-contract verification.',
    workflowObserve: 'Observe',
    workflowDecide: 'Decide',
    workflowAct: 'Act',
    workflowProve: 'Prove',
    boundaryTitle: 'Models may advise. They cannot authorize for you.',
    boundaryBody:
      'Deterministic facts, model advice, human decisions, and workspace mutation occupy four separate authority layers.',
    boundaryFacts: 'Facts',
    boundaryFactsBody:
      'Browser observations, Test Kit context, file digests, and local assertions.',
    boundaryAdvice: 'Advice',
    boundaryAdviceBody:
      'Grounding, design audit, and source-to-contract candidates stay traceable and non-blocking by default.',
    boundaryHuman: 'Authorization',
    boundaryHumanBody:
      'A person confirms targets, batches, conflict decisions, and repair intent.',
    boundaryRepair: 'Execution',
    boundaryRepairBody:
      'The session-owning coding agent edits source and a fresh browser run verifies it.',
    surfacesTitle: 'Cross-surface execution with one result contract',
    surfacesBody:
      'Web, GUI, and TUI adapters own their runtimes. The core never selects a concrete backend.',
    surfaceWeb: 'Web',
    surfaceWebBody:
      'Persistent agent sessions and ACL suites through A3S Browser or a compatible standalone browser.',
    surfaceGui: 'GUI',
    surfaceGuiBody:
      'Contract-tested on macOS through locked A3S CUA and release-certified on a real host.',
    surfaceTui: 'TUI',
    surfaceTuiBody:
      'ACL suites through owned PTY / ConPTY process trees and bounded terminal semantics.',
    ctaTitle: 'Prove one real workflow first',
    ctaBody:
      'Start with a persistent Web session, then preserve the verified path as ACL.',
    quickStart: 'Read the quick start',
    architecture: 'See architecture boundaries',
    footer: 'Typed actions, inspectable evidence, owned cleanup.',
  },
};
