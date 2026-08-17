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
  architecture: string;
  footer: string;
  experience: ExperienceCopy;
};

export const homeCopy: Record<Locale, LocalizedCopy> = {
  zh: {
    heroTitle: ['让 Agent 看懂页面', '让每次修复都有证据'],
    heroBody:
      'Test Kit 把页面渲染后的语义、组件、定位器和坐标绑定到同一次观察。你点选问题并决定发送范围，编码 Agent 按范围修复，A3S Test 再用新观察和断言验收。',
    startExperience: '在页面上标记问题',
    readDocs: '查看快速开始',
    installTitle: '一条命令安装 CLI 与 Agent Skill',
    installBody: '选择系统并复制命令，即可为本机安装匹配版本的 CLI 与 Skill。',
    installTabs: '选择安装平台',
    installPackage: 'CLI + Agent Skill',
    installNote: '安装器校验发布归档的 SHA-256，并保持 CLI 与 Skill 版本一致。',
    installCandidateNote:
      '当前文档已进入下一版本准备阶段。此命令仍固定安装已发布的稳定版。',
    copy: '复制命令',
    copied: '命令已复制',
    copyError: '复制失败，请手动选择命令',
    proofTitle: '先建立期望，再核对真实页面',
    proofBody:
      'PRD 回答产品应该做什么，设计稿说明界面应该是什么样，Test Kit 记录浏览器实际渲染了什么。前两类证据经人工审阅成为界面契约，A3S Test 再与当前页面逐项比对。',
    contractPanelLabel: '从来源到可核对契约',
    contractExpectedLabel: '期望证据',
    contractObservedLabel: '浏览器证据',
    contractPrdTitle: '产品意图',
    contractPrdBody: '功能结果、文案、约束和待确认事项。',
    contractDesignTitle: '视觉期望',
    contractDesignBody: '区域层级、视觉关系、尺寸与位置。',
    contractPageTitle: '渲染事实',
    contractPageBody: '当前修订中的语义、状态、组件、定位器与几何。',
    contractReviewTitle: '人工审阅',
    contractReviewBody: '确认引用、冲突和每条期望。',
    contractCompareTitle: '逐项比对',
    contractCompareBody: '用当前页面修订核对获准期望。',
    contractReportTitle: '可复查差异',
    contractReportBody: '保留来源、决定与页面证据。',
    contractDisclaimer:
      'PRD 和设计稿只提供期望来源，不会冒充浏览器可访问树。每条契约都保留来源、审阅决定和内容摘要。',
    contractGuide: '界面契约如何工作',
    capabilitiesTitle: '从点选问题到验证结果，五步闭环',
    capabilitiesBody:
      '页面负责提供事实，人负责确认修改意图，编码 Agent 只处理已发送的问题，A3S Test 用新观察判断结果。',
    capabilities: [
      {
        title: '读取渲染事实',
        body: '页面渲染后读取 DOM、可访问语义、交互状态、布局和组件归属。',
        code: '01 · perceive',
      },
      {
        title: '绑定当前目标',
        body: '把角色、名称、稳定定位器、组件来源和坐标绑定到当前页面修订。',
        code: '02 · target',
      },
      {
        title: '由人确认发送',
        body: '你确认单个或一批目标与修改要求。只有发送以后，修复权限才生效。',
        code: '03 · submit',
      },
      {
        title: '在授权范围修复',
        body: '编码 Agent 只处理已发送的问题，并回报修改文件与针对性检查。',
        code: '04 · repair',
      },
      {
        title: '用新观察验收',
        body: 'A3S Test 重新读取页面，用新的观察、断言和证据确认结果。',
        code: '05 · verify',
      },
    ],
    workflowTitle: 'Agent 会话探索未知，ACL 固化已知',
    workflowBody:
      '遇到陌生页面，用持久会话逐步观察、决策和动作。路径与成功条件确定后，再写成 ACL，交给本地和 CI 稳定复现。',
    workflowAgent: '用 Agent 会话探索',
    workflowAgentBody:
      '每轮从新观察开始，适合复现问题、理解陌生页面和试走新流程。',
    workflowAcl: '用 ACL 套件回归',
    workflowAclBody:
      '固定已经跑通的动作与断言，用于本地回归、CI 和界面契约验证。',
    workflowAgentLink: '了解 Agent 会话',
    workflowAclLink: '编写 ACL 套件',
    workflowObserve: '观察',
    workflowDecide: '决策',
    workflowAct: '动作',
    workflowProve: '证据',
    boundaryTitle: '标记不等于授权，发送才进入修复',
    boundaryBody:
      '浏览器事实、模型建议、人工授权和源码修改分开记录。查看建议、标记元素或保存草稿都不会改代码；只有明确发送的目标和要求才进入修复队列。',
    boundaryFacts: '页面事实',
    boundaryFactsBody: 'DOM、可访问语义、页面状态、几何与本地断言。',
    boundaryAdvice: '模型候选',
    boundaryAdviceBody: '视觉定位和设计审查只提供带来源与置信度的候选。',
    boundaryHuman: '人工授权',
    boundaryHumanBody: '你确认目标、修改要求，以及要单独发送还是组成批次。',
    boundaryRepair: '修复、验证与确认',
    boundaryRepairBody:
      '编码 Agent 修改授权范围内的源码，A3S Test 用新页面证据验证，再由人决定是否接受。',
    surfacesTitle: '一套结果契约，覆盖三类界面',
    surfacesBody:
      'Web、GUI 和 TUI 共享动作、证据与结果格式；每类界面仍由自己的驱动安全执行。',
    surfaceWeb: 'Web',
    surfaceWebBody:
      '用持久 Agent 会话探索，用 ACL 套件回归；通过 A3S Browser 或兼容的独立浏览器执行。',
    surfaceGui: 'GUI',
    surfaceGuiBody:
      'macOS 使用锁定的 CUA 契约并经过真实主机发布验证。Windows 与 Linux 后端仍需单独审核。',
    surfaceTui: 'TUI',
    surfaceTuiBody: '通过自有 PTY / ConPTY 进程树和有界终端语义运行 ACL 套件。',
    ctaTitle: '从一个页面开始',
    ctaBody:
      '在开发环境挂载 Test Kit，让 A3S Test 读取页面并标记一个真实问题。流程验证后，再将路径保存为 ACL 持续回归。',
    quickStart: '接入 Test Kit',
    architecture: '理解架构边界',
    footer: '新观察、受限动作、可复查证据。',
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
      reviewBody: '先点选目标，再写明期望结果。可单独保存，也可整理为批次。',
      openReview: '打开点选工具',
      reviewStarted: '点选工具已打开',
      live: '实时',
      localOnly:
        '本地演示：问题只保存在当前页面，不修改代码，也不会发送给 Agent。',
      evidenceTitle: '本地记录',
      evidenceWaiting: '等待保存',
      evidenceReady: '已保存',
      receiptId: '任务 ID',
      receiptStatus: '保存位置',
      receiptFindings: '问题',
      receiptMemory: '当前页面',
      receiptIdle: '未保存',
      findingUnit: '个问题',
      findingsUnit: '个问题',
      noFinding: '还没有问题',
      renderedStatus: '渲染完成',
      contextStatus: '上下文已绑定',
      evidenceStatus: '本地保存',
      motionSteps: ['读取页面', '绑定目标', '人工说明', '整理批次', '保存本地'],
      scanSummary: 'DOM · 语义 · 坐标',
      targetMarker: '目标',
      motionFinding: '已定位“提交订单”',
      motionRequest: '提高按钮文字对比度，同时保持布局不变。',
      motionPacket: '1 个问题 · 绑定当前上下文',
      motionContext: '实时上下文',
      motionContextValue: 'button · 提交订单',
      motionAdd: '加入批次',
      motionSend: '保存到本页',
      motionReady: '问题已保存到本页',
      motionPause: '暂停自动演示',
      motionResume: '继续自动演示',
    },
  },
  en: {
    heroTitle: ['Give agents the rendered context.', 'Prove every change.'],
    heroBody:
      'Test Kit binds rendered semantics, components, locators, and geometry to one observation. Mark an issue and choose what to send; a coding agent repairs only that scope, then A3S Test verifies the result with a fresh observation and assertions.',
    startExperience: 'Mark an issue on the page',
    readDocs: 'Open the quick start',
    installTitle: 'Install the CLI and Agent Skill in one command',
    installBody:
      'Choose your system and copy the command to install matching CLI and Skill versions.',
    installTabs: 'Choose an installation platform',
    installPackage: 'CLI + Agent Skill',
    installNote:
      'The installer verifies the release archive SHA-256 and keeps the CLI and Skill on the same version.',
    installCandidateNote:
      'The next documentation version is staged. This command still pins the published stable release.',
    copy: 'Copy command',
    copied: 'Command copied',
    copyError: 'Copy failed. Select the command manually.',
    proofTitle: 'Define the expected UI. Compare the rendered one.',
    proofBody:
      'A PRD says what the product should do, a design shows how the interface should look, and Test Kit records what the browser actually rendered. Human-reviewed expectations become a surface contract that A3S Test compares with the current page.',
    contractPanelLabel: 'From source to reviewable contract',
    contractExpectedLabel: 'Expected evidence',
    contractObservedLabel: 'Browser evidence',
    contractPrdTitle: 'Product intent',
    contractPrdBody: 'Outcomes, copy, constraints, and open decisions.',
    contractDesignTitle: 'Visual expectation',
    contractDesignBody:
      'Regions, visual relationships, dimensions, and position.',
    contractPageTitle: 'Rendered fact',
    contractPageBody:
      'Semantics, state, components, locators, and geometry in this revision.',
    contractReviewTitle: 'Human review',
    contractReviewBody: 'Confirm citations, conflicts, and every expectation.',
    contractCompareTitle: 'Field-by-field comparison',
    contractCompareBody:
      'Check approved expectations against this page revision.',
    contractReportTitle: 'Reviewable difference',
    contractReportBody: 'Retain sources, decisions, and page evidence.',
    contractDisclaimer:
      'PRDs and designs provide expected evidence; they never masquerade as a browser accessibility tree. Every contract keeps its source, review decision, and content digest.',
    contractGuide: 'How surface contracts work',
    capabilitiesTitle: 'Five steps from marked issue to verified result',
    capabilitiesBody:
      'The page supplies facts, a person confirms intent, the coding agent handles only submitted issues, and A3S Test judges the result from a fresh observation.',
    capabilities: [
      {
        title: 'Read rendered facts',
        body: 'After rendering, read DOM, accessible semantics, interaction state, layout, and component ownership.',
        code: '01 · perceive',
      },
      {
        title: 'Bind the current target',
        body: 'Bind role, name, stable locator, component source, and geometry to the current page revision.',
        code: '02 · target',
      },
      {
        title: 'Confirm and send',
        body: 'Confirm one target or a batch and the requested change. Repair authority begins only after sending.',
        code: '03 · submit',
      },
      {
        title: 'Repair within authority',
        body: 'The coding agent addresses only submitted issues and reports changed files and focused checks.',
        code: '04 · repair',
      },
      {
        title: 'Verify from a fresh observation',
        body: 'A3S Test reads the page again and checks the result with fresh observations, assertions, and evidence.',
        code: '05 · verify',
      },
    ],
    workflowTitle: 'Agent sessions explore. ACL preserves what works.',
    workflowBody:
      'On an unfamiliar page, use a persistent session to observe, decide, and act one step at a time. Once the path and success condition are clear, encode them as ACL for stable local and CI runs.',
    workflowAgent: 'Explore with an agent session',
    workflowAgentBody:
      'Start each turn from a fresh observation to reproduce a bug, understand an unfamiliar page, or try a new flow.',
    workflowAcl: 'Regress with an ACL suite',
    workflowAclBody:
      'Fix proven actions and assertions for local regression, CI, and stable interface-contract checks.',
    workflowAgentLink: 'Learn agent sessions',
    workflowAclLink: 'Write an ACL suite',
    workflowObserve: 'Observe',
    workflowDecide: 'Decide',
    workflowAct: 'Act',
    workflowProve: 'Evidence',
    boundaryTitle: 'Marking is not authority. Sending enters repair.',
    boundaryBody:
      'Browser facts, model advice, human authorization, and source edits remain separate. Viewing advice, marking an element, or saving a draft cannot change code; only explicitly sent targets and requests enter the repair queue.',
    boundaryFacts: 'Page facts',
    boundaryFactsBody:
      'DOM, accessible semantics, page state, geometry, and local assertions.',
    boundaryAdvice: 'Model candidates',
    boundaryAdviceBody:
      'Visual grounding and design review only return candidates with provenance and confidence.',
    boundaryHuman: 'Human authorization',
    boundaryHumanBody:
      'You confirm the target, requested change, and whether to send it alone or in a batch.',
    boundaryRepair: 'Repair, verification, and review',
    boundaryRepairBody:
      'The coding agent edits authorized source, A3S Test verifies with fresh page evidence, and the reviewer decides whether to accept it.',
    surfacesTitle: 'One result contract across three interface types',
    surfacesBody:
      'Web, GUI, and TUI share action, evidence, and result formats while each keeps a driver designed for its own safety model.',
    surfaceWeb: 'Web',
    surfaceWebBody:
      'Explore with persistent agent sessions and regress with ACL suites through A3S Browser or a compatible standalone browser.',
    surfaceGui: 'GUI',
    surfaceGuiBody:
      'macOS uses a locked CUA contract and real-host release verification. Windows and Linux backends still require separate review.',
    surfaceTui: 'TUI',
    surfaceTuiBody:
      'ACL suites through owned PTY / ConPTY process trees and bounded terminal semantics.',
    ctaTitle: 'Start with one page',
    ctaBody:
      'Mount Test Kit in development, let A3S Test read the page, and mark one real issue. Once the flow is proven, preserve it as ACL for continuous regression.',
    quickStart: 'Integrate Test Kit',
    architecture: 'Understand the boundaries',
    footer: 'Fresh observations. Bounded actions. Reviewable evidence.',
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
        'Select a target, then describe the expected result. Save one issue or collect a batch.',
      openReview: 'Open the marking tool',
      reviewStarted: 'Marking tool is open',
      live: 'LIVE',
      localOnly:
        'Local demo: issues stay in this page. Nothing edits source or sends work to an agent.',
      evidenceTitle: 'Local record',
      evidenceWaiting: 'Waiting to save',
      evidenceReady: 'Saved',
      receiptId: 'TASK ID',
      receiptStatus: 'STORAGE',
      receiptFindings: 'FINDINGS',
      receiptMemory: 'this page',
      receiptIdle: 'not saved',
      findingUnit: 'issue',
      findingsUnit: 'issues',
      noFinding: 'No issues yet',
      renderedStatus: 'Render complete',
      contextStatus: 'Context bound',
      evidenceStatus: 'Saved locally',
      motionSteps: [
        'Read page',
        'Bind target',
        'Describe issue',
        'Build batch',
        'Save in page',
      ],
      scanSummary: 'DOM · A11Y · XY',
      targetMarker: 'target',
      motionFinding: '“Place order” located',
      motionRequest: 'Increase label contrast without changing the layout.',
      motionPacket: '1 issue · current context bound',
      motionContext: 'Live context',
      motionContextValue: 'button · Place order',
      motionAdd: 'Add to batch',
      motionSend: 'Save in page',
      motionReady: 'Issue saved in this page',
      motionPause: 'Pause walkthrough',
      motionResume: 'Resume walkthrough',
    },
  },
};
