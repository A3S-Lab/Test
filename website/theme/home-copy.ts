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
    heroTitle: ['让 Agent 看清页面', '让每次动作都有证据'],
    heroBody:
      'A3S Test 让编码 Agent 依据最新页面观察执行受限动作，并在同一次运行中保存断言、截图和页面证据。需要组件归属、稳定定位器、真实坐标或人工标记时，再按需接入 Test Kit。',
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
    proofTitle: '核对产品期望与页面事实',
    proofBody:
      'PRD 定义要实现的用户结果，设计稿约束界面呈现，Test Kit 捕获当前浏览器修订已经渲染的事实。人工先审阅前两类候选，再发布 Surface Contract。A3S Test 随后核对当前页面，并为每个差异保留来源与证据。',
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
    capabilitiesTitle: '每个动作都从最新观察开始',
    capabilitiesBody:
      '页面变化后，旧引用立即失效。A3S Test 重新读取页面，再用断言和证据判断成功条件；跑通的路径才能写成 ACL 回归。',
    capabilities: [
      {
        title: '观察当前页面',
        body: '读取 DOM、可访问语义、交互状态、布局与组件归属，并生成新的观察编号。',
        code: '01 · observe',
      },
      {
        title: '绑定可操作目标',
        body: '优先使用角色、名称和稳定定位器，并保留组件来源与多坐标空间几何。',
        code: '02 · locate',
      },
      {
        title: '执行一个受限动作',
        body: '动作经过 Schema、策略和观察修订校验；页面变化后，旧引用立即失效。',
        code: '03 · act',
      },
      {
        title: '重新读取变化后的页面',
        body: '动作执行后获取新的观察编号与页面修订，旧引用不能继续使用。',
        code: '04 · reobserve',
      },
      {
        title: '断言结果并保存证据',
        body: '检查成功条件并保存报告；通过验证的路径可以固化为 ACL。',
        code: '05 · prove',
      },
    ],
    workflowTitle: '用会话走通，用 ACL 回归',
    workflowBody:
      'Agent 会话保留浏览器状态，让编码 Agent 根据最新观察决定下一步。流程稳定后，把最小可用路径写成 ACL，在本地和 CI 重复运行。',
    workflowAgent: '探索还没走通的流程',
    workflowAgentBody:
      '每次从最新观察选择一个动作，用于复现问题、探索陌生页面和确认成功条件。',
    workflowAcl: '用 ACL 回归已确认的流程',
    workflowAclBody:
      '把动作、等待与断言写成类型化套件，在本地、CI 和界面契约检查中重复运行。',
    workflowAgentLink: '了解 Agent 会话',
    workflowAclLink: '编写 ACL 套件',
    workflowObserve: '最新观察',
    workflowDecide: '类型化动作',
    workflowAct: '重新观察',
    workflowProve: '断言与证据',
    boundaryTitle: '模型给出候选，人工授权修复',
    boundaryBody:
      '浏览器提供当前事实，模型提出带来源的候选。人确认目标和发送范围后，拥有工作区的编码 Agent 才能修改源码；A3S Test 再用本地断言和新页面证据验收。',
    boundaryFacts: '浏览器事实',
    boundaryFactsBody: '当前 DOM、可访问语义、状态、几何、组件与断言结果。',
    boundaryAdvice: '模型建议',
    boundaryAdviceBody:
      '视觉定位和设计审查给出带来源、置信度和预算的候选，不决定测试结果。',
    boundaryHuman: '人工授权',
    boundaryHumanBody: '确认目标、期望结果、冲突关系，以及单项或批量发送范围。',
    boundaryRepair: '修改与验收',
    boundaryRepairBody:
      '编码 Agent 只在授权范围内修改，A3S Test 用新浏览器验证，再交回人工验收。',
    surfacesTitle: 'Web、GUI 与 TUI 共用核心契约',
    surfacesBody:
      'Core 统一动作、策略、证据和结果。每类界面的感知、执行和进程清理由专用驱动负责，支持范围以各驱动的验证状态为准。',
    surfaceWeb: 'Web',
    surfaceWebBody:
      'Agent 会话和 ACL 套件已通过 A3S Browser 或兼容的独立浏览器执行。',
    surfaceGui: 'GUI',
    surfaceGuiBody:
      'macOS CUA 已在真实 arm64 主机验证感知、权限和清理。Windows 与 Linux 后端仍在独立审核。',
    surfaceTui: 'TUI',
    surfaceTuiBody:
      'ACL 套件通过自有 PTY / ConPTY 进程树运行，并使用有界终端语义和清理规则。',
    ctaTitle: '从一个真实页面开始',
    ctaBody:
      '安装 CLI，启动一个带可观察成功条件的 Agent 会话。需要组件、坐标或人工标记时，再接入 Test Kit。流程跑通后，把它写成 ACL 回归。',
    quickStart: '运行第一个测试',
    testkitGuide: '接入 Test Kit',
    architecture: '查看架构',
    footer: '依据当前页面行动，把结果留成可复查证据。',
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
      reviewTitle: '标记页面问题',
      reviewBody: '点选元素并写清期望结果。多个相关问题可以一起整理后保存。',
      openReview: '开始标记',
      reviewStarted: '标记工具已打开',
      live: '实时',
      localOnly:
        '此演示只把问题保存在当前标签页，不连接修复 Agent，也不会修改源码。',
      evidenceTitle: '本页问题',
      evidenceWaiting: '尚未保存',
      evidenceReady: '已保存到本页',
      receiptId: '记录 ID',
      receiptStatus: '保存位置',
      receiptFindings: '问题数量',
      receiptMemory: '当前标签页',
      receiptIdle: '尚未保存',
      findingUnit: '个问题',
      findingsUnit: '个问题',
      noFinding: '尚未保存问题',
      renderedStatus: '渲染完成',
      contextStatus: '上下文已绑定',
      evidenceStatus: '本地保存',
      motionSteps: ['读取页面', '绑定目标', '说明问题', '组成批次', '保存本页'],
      scanSummary: 'DOM · 语义 · 坐标',
      targetMarker: '目标',
      motionFinding: '已绑定“提交订单”',
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
    heroTitle: ['Observe the page.', 'Prove every action.'],
    heroBody:
      'A3S Test lets a coding agent act from a fresh page observation and keeps assertions, screenshots, and page evidence within the same run. Add Test Kit when a workflow needs component ownership, stable locators, rendered geometry, or human marking.',
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
    proofTitle: 'Compare expectations with rendered facts',
    proofBody:
      'A PRD defines the intended outcome, a design constrains presentation, and Test Kit captures what the current browser revision rendered. A person reviews the first two sources before publishing a Surface Contract. A3S Test then checks the current page and keeps provenance and evidence for every difference.',
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
    capabilitiesTitle: 'Every action starts from a fresh observation',
    capabilitiesBody:
      'A page change expires old refs immediately. A3S Test observes again before it evaluates the success condition or preserves a proven path as ACL.',
    capabilities: [
      {
        title: 'Observe the current page',
        body: 'Read DOM, accessible semantics, interaction state, layout, and component ownership, then issue a fresh observation ID.',
        code: '01 · observe',
      },
      {
        title: 'Bind an actionable target',
        body: 'Prefer role, name, and stable locators while retaining component source and multi-space geometry.',
        code: '02 · locate',
      },
      {
        title: 'Execute one bounded action',
        body: 'Validate schema, policy, and observation revision. Page changes immediately expire old refs.',
        code: '03 · act',
      },
      {
        title: 'Observe the changed page again',
        body: 'Issue a new observation ID and page revision after the action. Old refs cannot be reused.',
        code: '04 · reobserve',
      },
      {
        title: 'Assert the result and retain evidence',
        body: 'Check the success condition and retain the report. A verified path can then become an ACL regression.',
        code: '05 · prove',
      },
    ],
    workflowTitle: 'Explore in a session. Repeat with ACL.',
    workflowBody:
      'An agent session keeps browser state while a coding agent chooses the next action from the latest observation. Once the flow is stable, encode its smallest useful path as ACL for local and CI runs.',
    workflowAgent: 'Explore an unproven flow',
    workflowAgentBody:
      'Choose one action from each fresh observation to reproduce a bug, explore an unfamiliar page, and confirm the success condition.',
    workflowAcl: 'Repeat a proven flow with ACL',
    workflowAclBody:
      'Encode actions, waits, and assertions as a typed suite that runs locally, in CI, and during Surface Contract checks.',
    workflowAgentLink: 'Learn agent sessions',
    workflowAclLink: 'Write an ACL suite',
    workflowObserve: 'Fresh observation',
    workflowDecide: 'Typed action',
    workflowAct: 'Observe again',
    workflowProve: 'Assertions and evidence',
    boundaryTitle: 'Models propose candidates. People authorize repair.',
    boundaryBody:
      'The browser supplies current facts. Models may propose provenance-bound candidates, but a person chooses the target and submission scope. Only a workspace-owning coding agent may edit source, and A3S Test must verify the result in a fresh browser.',
    boundaryFacts: 'Browser facts',
    boundaryFactsBody:
      'Current DOM, accessible semantics, state, geometry, components, and assertion results.',
    boundaryAdvice: 'Model advice',
    boundaryAdviceBody:
      'Visual grounding and design review return candidates with provenance, confidence, and budgets. They never set the verdict.',
    boundaryHuman: 'Human authorization',
    boundaryHumanBody:
      'Confirm the target, expected result, conflicts, and the single or batch submission scope.',
    boundaryRepair: 'Edits and acceptance',
    boundaryRepairBody:
      'The coding agent edits only within scope, A3S Test verifies in a fresh browser, and the reviewer accepts or reopens the result.',
    surfacesTitle: 'Web, GUI, and TUI share one Core contract',
    surfacesBody:
      'Core unifies actions, policy, evidence, and results. A dedicated driver owns perception, execution, and process cleanup for each surface, with support limited to its verified status.',
    surfaceWeb: 'Web',
    surfaceWebBody:
      'Agent sessions and ACL suites run through A3S Browser or a compatible standalone browser.',
    surfaceGui: 'GUI',
    surfaceGuiBody:
      'macOS CUA perception, permissions, and cleanup are verified on a real arm64 host. Windows and Linux backends remain under separate review.',
    surfaceTui: 'TUI',
    surfaceTuiBody:
      'ACL suites run through owned PTY / ConPTY process trees with bounded terminal semantics and cleanup.',
    ctaTitle: 'Start with one real page',
    ctaBody:
      'Install the CLI and start an agent session with an observable success condition. Add Test Kit only when you need components, geometry, or human marking. Once the flow is proven, preserve it as ACL.',
    quickStart: 'Run your first test',
    testkitGuide: 'Add Test Kit',
    architecture: 'Review architecture',
    footer: 'Act from the current page. Keep the result reviewable.',
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
      reviewTitle: 'Mark a page issue',
      reviewBody:
        'Select an element and state the expected result. Save related issues as one batch.',
      openReview: 'Start marking',
      reviewStarted: 'Marking is active',
      live: 'LIVE',
      localOnly:
        'This demo keeps issues in the current tab. It does not connect to a repair agent or edit source.',
      evidenceTitle: 'Issues on this page',
      evidenceWaiting: 'Nothing saved',
      evidenceReady: 'Saved in this tab',
      receiptId: 'RECORD ID',
      receiptStatus: 'SAVED IN',
      receiptFindings: 'ISSUES',
      receiptMemory: 'current tab',
      receiptIdle: 'nothing saved',
      findingUnit: 'issue',
      findingsUnit: 'issues',
      noFinding: 'No saved issues',
      renderedStatus: 'Render complete',
      contextStatus: 'Context bound',
      evidenceStatus: 'Saved locally',
      motionSteps: [
        'Read page',
        'Bind target',
        'Describe issue',
        'Build batch',
        'Save in tab',
      ],
      scanSummary: 'DOM · A11Y · XY',
      targetMarker: 'target',
      motionFinding: '“Place order” bound',
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
