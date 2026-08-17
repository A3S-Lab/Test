export type Locale = 'zh' | 'en';

export type ExperienceCopy = {
  stageAria: string;
  boundaryName: string;
  sample: string;
  shop: string;
  checkoutTitle: string;
  checkoutProgress: [string, string, string, string];
  customerTitle: string;
  customerName: string;
  customerAddress: string;
  productsTitle: string;
  productName: string;
  productVariant: string;
  quantity: string;
  subtotal: string;
  totalDue: string;
  summaryTitle: string;
  productTotal: string;
  delivery: string;
  deliveryValue: string;
  discount: string;
  discountValue: string;
  payable: string;
  paymentTitle: string;
  paymentMethod: string;
  submit: string;
  submitted: string;
  contextTitle: string;
  refresh: string;
  connecting: string;
  contextUnavailable: string;
  notAvailable: string;
  selected: string;
  revision: string;
  role: string;
  name: string;
  geometry: string;
  locator: string;
  source: string;
  reviewTitle: string;
  reviewBody: string;
  openReview: string;
  reviewStarted: string;
  live: string;
  localOnly: string;
  evidenceTitle: string;
  evidenceWaiting: string;
  evidenceReady: string;
  receiptId: string;
  receiptStatus: string;
  receiptFindings: string;
  receiptMemory: string;
  receiptIdle: string;
  findingUnit: string;
  findingsUnit: string;
  noFinding: string;
  renderedStatus: string;
  contextStatus: string;
  evidenceStatus: string;
  motionSteps: [string, string, string, string, string];
  scanSummary: string;
  targetMarker: string;
  motionFinding: string;
  motionRequest: string;
  motionPacket: string;
  motionContext: string;
  motionContextValue: string;
  motionAdd: string;
  motionSend: string;
  motionReady: string;
  motionPause: string;
  motionResume: string;
};

type LocalizedCopy = {
  heroTitle: [string, string];
  heroBody: string;
  startExperience: string;
  readDocs: string;
  installTitle: string;
  installBody: string;
  installTabs: string;
  installPackage: string;
  installNote: string;
  installCandidateNote: string;
  copy: string;
  copied: string;
  copyError: string;
  proofTitle: string;
  proofBody: string;
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
  contractGuide: string;
  capabilitiesTitle: string;
  capabilitiesBody: string;
  capabilities: Array<{ title: string; body: string; code: string }>;
  workflowTitle: string;
  workflowBody: string;
  workflowAgent: string;
  workflowAgentBody: string;
  workflowAcl: string;
  workflowAclBody: string;
  workflowAgentLink: string;
  workflowAclLink: string;
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
  testkitGuide: string;
  architecture: string;
  footer: string;
  experience: ExperienceCopy;
};

export const homeCopy: Record<Locale, LocalizedCopy> = {
  zh: {
    heroTitle: ['让 Agent 看清页面', '让每一步都能复现和验收'],
    heroBody:
      'A3S Test 为编码 Agent 保留浏览器会话，让每个动作都绑定最新页面观察，并把结果写入可复查证据。接入 Test Kit 后，还能读取组件归属、稳定定位器、真实坐标与 UI 证据，并把人工点选的单个或批量问题明确送入修复。',
    startExperience: '在本页标记一个问题',
    readDocs: '运行第一个测试',
    installTitle: '一条命令，安装 CLI 与 Agent Skill',
    installBody:
      '选择 macOS、Linux 或 Windows，复制命令后即可安装相同版本的 CLI 与 Agent Skill。',
    installTabs: '选择安装平台',
    installPackage: 'CLI + Agent Skill',
    installNote: '安装器校验发布归档的 SHA-256，并保持 CLI 与 Skill 版本一致。',
    installCandidateNote:
      '当前文档已进入下一版本准备阶段。此命令仍固定安装已发布的稳定版。',
    copy: '复制命令',
    copied: '命令已复制',
    copyError: '复制失败，请手动选择命令',
    proofTitle: '把产品期望与真实页面逐项核对',
    proofBody:
      'PRD 说明产品应该完成什么，设计稿说明界面应该如何呈现，Test Kit 记录浏览器实际渲染了什么。人工审阅前两类候选后，A3S Test 把它们发布为 Surface Contract，再与当前页面修订逐项核对并保存差异证据。',
    contractPanelLabel: '产品期望与当前页面的核对路径',
    contractExpectedLabel: '产品期望',
    contractObservedLabel: '当前事实',
    contractPrdTitle: '应该完成什么',
    contractPrdBody: '用户结果、文案、业务约束与尚待决定的事项。',
    contractDesignTitle: '应该如何呈现',
    contractDesignBody: '区域层级、相互关系、尺寸、位置与视觉约束。',
    contractPageTitle: '现在实际呈现什么',
    contractPageBody: '本次修订的语义、状态、组件、定位器与几何。',
    contractReviewTitle: '人工确认候选',
    contractReviewBody: '选择可采纳的要求，处理冲突与未决事项。',
    contractCompareTitle: '核对真实页面',
    contractCompareBody: '逐项检查已确认期望与真实页面。',
    contractReportTitle: '保留可复查差异',
    contractReportBody: '记录来源、决定、页面修订与证据。',
    contractDisclaimer:
      'PRD 和设计稿产生的是期望候选，不是浏览器可访问树。只有经人工审阅的候选才能进入 Surface Contract，页面事实仍由当前浏览器修订提供。',
    contractGuide: '了解界面契约如何生成',
    capabilitiesTitle: '从观察到验收，每一步都有证据',
    capabilitiesBody:
      'A3S Test 不靠固定等待猜测页面状态，也不让旧引用跨越页面变化。一次完整测试从最新观察开始，以新页面证据结束。',
    capabilities: [
      {
        title: '观察当前页面',
        body: '读取 DOM、可访问语义、交互状态、布局与组件归属，并生成新的观察编号。',
        code: '01 · observe',
      },
      {
        title: '锁定可操作目标',
        body: '优先使用角色、名称和稳定定位器，并保留组件来源与多坐标空间几何。',
        code: '02 · locate',
      },
      {
        title: '执行一个受限动作',
        body: '动作经过 Schema、策略和观察修订校验；页面变化后，旧引用立即失效。',
        code: '03 · act',
      },
      {
        title: '由人确认修复范围',
        body: '点选单个或一批问题，补充期望结果，明确发送后才授予修复权限。',
        code: '04 · authorize',
      },
      {
        title: '用新页面证明结果',
        body: '重新观察页面，运行断言并保存报告；跑通的路径可以固化为 ACL。',
        code: '05 · prove',
      },
    ],
    workflowTitle: '未知路径先探索，稳定路径再回归',
    workflowBody:
      'Agent 会话适合一边观察一边决策；ACL 套件适合重复运行明确动作和断言。两者共用动作、驱动、证据与清理规则。',
    workflowAgent: '用 Agent 会话探索未知路径',
    workflowAgentBody:
      '每次只依据最新观察执行一个动作，适合复现问题、试走流程和理解陌生页面。',
    workflowAcl: '用 ACL 重复验证已知',
    workflowAclBody:
      '把已确认的动作、等待与断言写成类型化套件，用于本地回归、CI 和界面契约检查。',
    workflowAgentLink: '了解 Agent 会话',
    workflowAclLink: '编写 ACL 套件',
    workflowObserve: '最新观察',
    workflowDecide: '类型化动作',
    workflowAct: '重新观察',
    workflowProve: '断言与证据',
    boundaryTitle: '模型可以建议，修改必须明确授权',
    boundaryBody:
      '浏览器提供事实，模型提出候选，人决定哪些问题可以发送，拥有工作区的编码 Agent 才能修改源码。修改后仍必须用本地断言和新页面证据验收。',
    boundaryFacts: '浏览器事实',
    boundaryFactsBody: '当前 DOM、可访问语义、状态、几何、组件与断言结果。',
    boundaryAdvice: '模型建议',
    boundaryAdviceBody: '视觉定位和设计审查给出带来源、置信度和预算的候选。',
    boundaryHuman: '人工授权',
    boundaryHumanBody: '确认目标、期望结果、冲突关系，以及单项或批量发送范围。',
    boundaryRepair: '修改与验收',
    boundaryRepairBody:
      '编码 Agent 在授权范围内修改，A3S Test 用新浏览器验证，再交回人工验收。',
    surfacesTitle: '同一套动作与证据契约，适配 Web、GUI 与 TUI',
    surfacesBody:
      '动作、策略、证据和结果共用一套 Core 契约；每类界面由专用驱动负责感知、执行和精确清理。',
    surfaceWeb: 'Web',
    surfaceWebBody:
      '用持久 Agent 会话探索，用 ACL 套件回归；通过 A3S Browser 或兼容的独立浏览器执行。',
    surfaceGui: 'GUI',
    surfaceGuiBody:
      'macOS 使用锁定的 CUA 契约，并在真实 arm64 主机验证感知、权限和清理。Windows 与 Linux 后端仍需单独审核。',
    surfaceTui: 'TUI',
    surfaceTuiBody: '通过自有 PTY / ConPTY 进程树和有界终端语义运行 ACL 套件。',
    ctaTitle: '从观察一个真实页面开始',
    ctaBody:
      '安装 CLI，启动一个带可观察成功条件的会话，再执行第一次观察。需要组件、坐标或人工点选时，再在开发环境接入 Test Kit；跑通后将稳定路径写成 ACL。',
    quickStart: '运行第一个测试',
    testkitGuide: '接入 Test Kit',
    architecture: '查看架构',
    footer: '看清当前页面，只做获准动作，保留可复查证据。',
    experience: {
      stageAria: 'A3S Test Kit 实时体验',
      boundaryName: '订单确认体验',
      sample: '交互演示',
      shop: 'A3S Shop',
      checkoutTitle: '确认订单',
      checkoutProgress: ['购物车', '确认订单', '支付', '完成'],
      customerTitle: '收货信息',
      customerName: '张三 · 138 0000 0000',
      customerAddress: '北京市海淀区中关村大街 1 号 A3S 大厦 1001 室',
      productsTitle: '商品清单',
      productName: 'A3S 自动化测试套件',
      productVariant: '团队版 · 年度许可',
      quantity: '数量 1',
      subtotal: '¥1,298.00',
      totalDue: '¥1,248.00',
      summaryTitle: '订单金额',
      productTotal: '商品总额',
      delivery: '运费',
      deliveryValue: '¥0.00',
      discount: '优惠',
      discountValue: '−¥50.00',
      payable: '实付金额',
      paymentTitle: '支付方式',
      paymentMethod: '企业测试账户',
      submit: '提交订单',
      submitted: '演示订单已提交，页面上下文已刷新。',
      contextTitle: '页面上下文',
      refresh: '刷新',
      connecting: '正在读取真实页面上下文…',
      contextUnavailable: '未读到页面上下文，请刷新重试。',
      notAvailable: '暂无',
      selected: '当前目标',
      revision: '页面修订',
      role: '角色',
      name: '名称',
      geometry: '视口几何',
      locator: '首选定位器',
      source: '组件来源',
      reviewTitle: '标记并说明问题',
      reviewBody:
        '点选一个元素并写明期望结果。需要时可以把多个问题整理为批次。',
      openReview: '开始点选',
      reviewStarted: '点选工具已打开',
      live: '实时',
      localOnly:
        '此演示只把问题保存在当前标签页，不连接修复 Agent，也不会修改源码。',
      evidenceTitle: '本页记录',
      evidenceWaiting: '等待保存',
      evidenceReady: '已保存',
      receiptId: '记录 ID',
      receiptStatus: '保存范围',
      receiptFindings: '问题',
      receiptMemory: '当前标签页',
      receiptIdle: '未保存',
      findingUnit: '个问题',
      findingsUnit: '个问题',
      noFinding: '还没有问题',
      renderedStatus: '渲染完成',
      contextStatus: '上下文已绑定',
      evidenceStatus: '本地保存',
      motionSteps: ['读取页面', '定位元素', '写下问题', '组成批次', '保存本页'],
      scanSummary: 'DOM · 语义 · 坐标',
      targetMarker: '目标',
      motionFinding: '已定位“提交订单”',
      motionRequest: '提高按钮文字对比度，保持尺寸与位置不变。',
      motionPacket: '1 个问题 · 已绑定当前修订',
      motionContext: '实时上下文',
      motionContextValue: 'button · 提交订单',
      motionAdd: '加入批次',
      motionSend: '保存到本页',
      motionReady: '问题已保存到当前标签页',
      motionPause: '暂停自动演示',
      motionResume: '继续自动演示',
    },
  },
  en: {
    heroTitle: ['Read the current page.', 'Prove every action.'],
    heroBody:
      'A3S Test keeps a browser session alive for a coding agent, binds every action to a fresh observation, and writes the outcome to inspectable evidence. Add Test Kit for component ownership, stable locators, real geometry, and UI evidence, or to send one marked issue or a batch into repair.',
    startExperience: 'Mark an issue on this page',
    readDocs: 'Run your first test',
    installTitle: 'Install the CLI and Agent Skill with one command',
    installBody:
      'Choose macOS, Linux, or Windows and copy one command to install matching CLI and Agent Skill versions.',
    installTabs: 'Choose an installation platform',
    installPackage: 'CLI + Agent Skill',
    installNote:
      'The installer verifies the release archive SHA-256 and keeps the CLI and Skill on the same version.',
    installCandidateNote:
      'The next documentation version is staged. This command still pins the published stable release.',
    copy: 'Copy command',
    copied: 'Command copied',
    copyError: 'Copy failed. Select the command manually.',
    proofTitle: 'Reconcile product expectations with the rendered page',
    proofBody:
      'A PRD says what the product should accomplish, a design says how the interface should appear, and Test Kit records what the browser actually rendered. After human review, A3S Test publishes the first two as a Surface Contract, checks it against the current page revision, and retains evidence for every difference.',
    contractPanelLabel: 'How product expectations meet the current page',
    contractExpectedLabel: 'Product expectation',
    contractObservedLabel: 'Current fact',
    contractPrdTitle: 'What should the product do?',
    contractPrdBody:
      'User outcomes, copy, business constraints, and open decisions.',
    contractDesignTitle: 'How should it appear?',
    contractDesignBody:
      'Regions, relationships, dimensions, position, and visual constraints.',
    contractPageTitle: 'What did this revision render?',
    contractPageBody:
      'Semantics, state, components, locators, and geometry from this page revision.',
    contractReviewTitle: 'Review the candidates',
    contractReviewBody:
      'Select admissible requirements and resolve conflicts or open decisions.',
    contractCompareTitle: 'Check the rendered page',
    contractCompareBody:
      'Compare every approved expectation with the rendered page.',
    contractReportTitle: 'Retain reviewable differences',
    contractReportBody:
      'Record sources, decisions, page revision, and evidence.',
    contractDisclaimer:
      'PRDs and designs produce expectation candidates, not a browser accessibility tree. Only human-reviewed candidates enter a Surface Contract, while the current browser revision remains the source of rendered facts.',
    contractGuide: 'See how interface contracts are built',
    capabilitiesTitle:
      'Every step from observation to acceptance leaves evidence',
    capabilitiesBody:
      'A3S Test does not guess page state with fixed delays or carry stale refs across page changes. A complete test starts from a fresh observation and ends with fresh page evidence.',
    capabilities: [
      {
        title: 'Observe the current page',
        body: 'Read DOM, accessible semantics, interaction state, layout, and component ownership, then issue a fresh observation ID.',
        code: '01 · observe',
      },
      {
        title: 'Resolve an actionable target',
        body: 'Prefer role, name, and stable locators while retaining component source and multi-space geometry.',
        code: '02 · locate',
      },
      {
        title: 'Execute one bounded action',
        body: 'Validate schema, policy, and observation revision. Page changes immediately expire old refs.',
        code: '03 · act',
      },
      {
        title: 'Have a person authorize repair',
        body: 'Mark one issue or a batch, describe the expected result, and grant repair authority only by sending it.',
        code: '04 · authorize',
      },
      {
        title: 'Prove the result on a fresh page',
        body: 'Observe again, run assertions, and retain the report. Proven paths can then become ACL regressions.',
        code: '05 · prove',
      },
    ],
    workflowTitle: 'Explore unknown paths. Regress stable ones.',
    workflowBody:
      'Agent sessions support observation-led decisions. ACL suites repeat explicit actions and assertions. Both use the same actions, drivers, evidence, and cleanup rules.',
    workflowAgent: 'Explore unknown paths with an agent session',
    workflowAgentBody:
      'Take one action from each fresh observation to reproduce a bug, try a flow, or understand an unfamiliar page.',
    workflowAcl: 'Repeat known paths with ACL',
    workflowAclBody:
      'Encode approved actions, waits, and assertions as a typed suite for local regression, CI, and Surface Contract checks.',
    workflowAgentLink: 'Learn agent sessions',
    workflowAclLink: 'Write an ACL suite',
    workflowObserve: 'Fresh observation',
    workflowDecide: 'Typed action',
    workflowAct: 'Observe again',
    workflowProve: 'Assertions and evidence',
    boundaryTitle: 'Models may advise. Source changes require authorization.',
    boundaryBody:
      'The browser supplies facts, models propose candidates, people decide what may be sent, and only a workspace-owning coding agent edits source. Local assertions and fresh page evidence must still accept the change.',
    boundaryFacts: 'Browser facts',
    boundaryFactsBody:
      'Current DOM, accessible semantics, state, geometry, components, and assertion results.',
    boundaryAdvice: 'Model advice',
    boundaryAdviceBody:
      'Visual grounding and design review return candidates with provenance, confidence, and budgets.',
    boundaryHuman: 'Human authorization',
    boundaryHumanBody:
      'Confirm the target, expected result, conflicts, and the single or batch submission scope.',
    boundaryRepair: 'Edits and acceptance',
    boundaryRepairBody:
      'The coding agent edits within scope, A3S Test verifies in a fresh browser, and the reviewer accepts or reopens the result.',
    surfacesTitle: 'One action and evidence contract across Web, GUI, and TUI',
    surfacesBody:
      'Actions, policy, evidence, and results share one Core contract. A dedicated driver owns perception, execution, and exact cleanup for each surface.',
    surfaceWeb: 'Web',
    surfaceWebBody:
      'Explore with persistent agent sessions and regress with ACL suites through A3S Browser or a compatible standalone browser.',
    surfaceGui: 'GUI',
    surfaceGuiBody:
      'macOS uses a locked CUA contract with perception, permissions, and cleanup verified on a real arm64 host. Windows and Linux backends still require separate review.',
    surfaceTui: 'TUI',
    surfaceTuiBody:
      'ACL suites through owned PTY / ConPTY process trees and bounded terminal semantics.',
    ctaTitle: 'Start with one real page observation',
    ctaBody:
      'Install the CLI, start a session with an observable success condition, and take the first observation. Add Test Kit when you need components, geometry, or human marking, then preserve the stable path as ACL.',
    quickStart: 'Run your first test',
    testkitGuide: 'Add Test Kit',
    architecture: 'Review architecture',
    footer:
      'Read the current page. Take approved actions. Retain reviewable evidence.',
    experience: {
      stageAria: 'Live A3S Test Kit experience',
      boundaryName: 'Checkout experience',
      sample: 'Interactive demo',
      shop: 'A3S Shop',
      checkoutTitle: 'Review order',
      checkoutProgress: ['Cart', 'Review', 'Payment', 'Complete'],
      customerTitle: 'Delivery details',
      customerName: 'Alex Chen · +1 555 0100',
      customerAddress: '1001 Interface Way, San Francisco, CA 94107',
      productsTitle: 'Order items',
      productName: 'A3S autonomous testing suite',
      productVariant: 'Team · annual license',
      quantity: 'Quantity 1',
      subtotal: '$1,298.00',
      totalDue: '$1,248.00',
      summaryTitle: 'Order summary',
      productTotal: 'Products',
      delivery: 'Delivery',
      deliveryValue: '$0.00',
      discount: 'Discount',
      discountValue: '−$50.00',
      payable: 'Total due',
      paymentTitle: 'Payment method',
      paymentMethod: 'Company test account',
      submit: 'Place order',
      submitted: 'Demo order submitted. Page context refreshed.',
      contextTitle: 'Page context',
      refresh: 'Refresh',
      connecting: 'Reading the real page context…',
      contextUnavailable: 'Page context is unavailable. Refresh to try again.',
      notAvailable: 'n/a',
      selected: 'Current target',
      revision: 'Page revision',
      role: 'Role',
      name: 'Name',
      geometry: 'Viewport box',
      locator: 'Primary locator',
      source: 'Component source',
      reviewTitle: 'Mark and describe an issue',
      reviewBody:
        'Select one element and describe the expected result. Collect a batch when several issues belong together.',
      openReview: 'Start marking',
      reviewStarted: 'Marking tool is open',
      live: 'LIVE',
      localOnly:
        'This demo keeps issues in the current tab. It does not connect to a repair agent or edit source.',
      evidenceTitle: 'Page record',
      evidenceWaiting: 'Waiting to save',
      evidenceReady: 'Saved',
      receiptId: 'RECORD ID',
      receiptStatus: 'SCOPE',
      receiptFindings: 'FINDINGS',
      receiptMemory: 'current tab',
      receiptIdle: 'not saved',
      findingUnit: 'issue',
      findingsUnit: 'issues',
      noFinding: 'No issues yet',
      renderedStatus: 'Render complete',
      contextStatus: 'Context bound',
      evidenceStatus: 'Saved locally',
      motionSteps: [
        'Read page',
        'Locate element',
        'Describe issue',
        'Build batch',
        'Save in tab',
      ],
      scanSummary: 'DOM · A11Y · XY',
      targetMarker: 'target',
      motionFinding: '“Place order” located',
      motionRequest:
        'Increase label contrast. Keep size and position unchanged.',
      motionPacket: '1 issue · current revision bound',
      motionContext: 'Live context',
      motionContextValue: 'button · Place order',
      motionAdd: 'Add to batch',
      motionSend: 'Save in tab',
      motionReady: 'Issue saved in the current tab',
      motionPause: 'Pause walkthrough',
      motionResume: 'Resume walkthrough',
    },
  },
};
