export type Locale = 'zh' | 'en';

export type ExperienceCopy = {
  stageAria: string;
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
  localOnly: string;
  evidenceTitle: string;
  evidenceWaiting: string;
  evidenceReady: string;
  findingsUnit: string;
  noFinding: string;
  renderedStatus: string;
  contextStatus: string;
  evidenceStatus: string;
  motionSteps: [string, string, string, string, string];
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
  viewGitHub: string;
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
  experience: ExperienceCopy;
};

export const homeCopy: Record<Locale, LocalizedCopy> = {
  zh: {
    heroTitle: ['先让 Agent 看清页面，', '再把问题交给它修'],
    heroBody:
      'Test Kit 在浏览器完成渲染后生成有界、带修订号的页面上下文。你可以点选单个或一批问题；A3S Test 会把目标、修改要求和现场证据交给获授权的编码 Agent，并在改动后重新验证。',
    startExperience: '在演示页标记问题',
    readDocs: '阅读快速开始',
    viewGitHub: '查看 GitHub',
    installTitle: '安装 CLI 与编码 Agent Skill',
    installBody:
      '选择当前平台，复制一条命令，为本机安装匹配版本的 CLI 与 Skill。',
    installTabs: '选择安装平台',
    installPackage: 'CLI + Agent Skill',
    installNote: '安装器校验发布归档的 SHA-256，并保持 CLI 与 Skill 版本一致。',
    installCandidateNote:
      '当前文档已进入下一版本准备阶段；此命令仍固定安装已发布的稳定版。',
    copy: '复制命令',
    copied: '已复制',
    copyError: '复制失败',
    proofTitle: '浏览器先渲染，Test Kit 再整理事实',
    proofBody:
      '它不重做浏览器，也不先猜截图。它把 DOM、可访问语义、组件来源、稳定定位器、盒模型和多坐标空间几何整理成同一份有界快照；页面变化后，旧引用立即失效。',
    sampleLabel: '页面上下文示例',
    observationLabel: '观察',
    revisionLabel: '界面修订',
    actionLabel: '类型化动作',
    evidenceLabel: '持久证据',
    pageContextLabel: '当前目标',
    pageContextValue: '@c7 button “Place order”',
    actionValue: 'click @c7 · observation 42',
    evidenceValue: 'events.jsonl + report.json',
    capabilitiesTitle: '从页面现场到验证结果，只走一条闭环',
    capabilitiesBody:
      '感知、定位、提交、修复和验证各有明确输入与权限。任何阶段都不能替前一阶段补写事实。',
    capabilities: [
      {
        title: '感知真实页面',
        body: '等待浏览器渲染，再读取 DOM、可访问语义、交互状态、布局和组件归属。',
        code: '01 · perceive',
      },
      {
        title: '精确绑定目标',
        body: '把角色、名称、稳定定位器、组件来源和坐标绑定到同一个页面修订。',
        code: '02 · target',
      },
      {
        title: '明确提交问题',
        body: '人工确认单个或一批目标与修改要求；只有提交才会授予修复权限。',
        code: '03 · submit',
      },
      {
        title: 'Agent 在范围内修复',
        body: '编码 Agent 只处理已授权的问题，并回报修改文件与针对性检查。',
        code: '04 · repair',
      },
      {
        title: '浏览器重新验证',
        body: 'A3S Test 获取更新后的页面上下文，用新证据确认目标问题是否解决。',
        code: '05 · verify',
      },
    ],
    workflowTitle: '先探索未知路径，再固化稳定路径',
    workflowBody:
      '持久 Agent 会话用于理解尚不确定的页面。路径和成功条件被证实后，再用同一套动作与证据写成 ACL。',
    workflowAgent: 'Agent 会话：探索当前页面',
    workflowAgentBody:
      '每轮都从新观察开始，适合复现问题、检查陌生页面和试走新流程。',
    workflowAcl: 'ACL 套件：重复已证实路径',
    workflowAclBody:
      '把关键动作与断言固定下来，用于本地回归、CI 和界面契约验证。',
    workflowObserve: '观察',
    workflowDecide: '决策',
    workflowAct: '动作',
    workflowProve: '证据',
    boundaryTitle: '提交是修复授权的边界',
    boundaryBody:
      '浏览器事实、可选建议、人工提交和源码修改分开记录。查看建议或保存草稿都不会授权改代码；只有明确提交的目标和要求会进入修复队列。',
    boundaryFacts: '页面事实',
    boundaryFactsBody: 'DOM、可访问语义、页面状态、几何与本地断言。',
    boundaryAdvice: '可选建议',
    boundaryAdviceBody: '视觉定位和设计审查只提供带来源与置信度的候选。',
    boundaryHuman: '人工提交',
    boundaryHumanBody: '你确认目标、修改要求，以及要单独发送还是组成批次。',
    boundaryRepair: '修复与验证',
    boundaryRepairBody:
      '编码 Agent 修改获授权的源码，A3S Test 再用新页面证据验证。',
    surfacesTitle: '核心契约一致，界面驱动各自负责',
    surfacesBody:
      'Web、GUI 和 TUI 不共享脆弱的模拟层；它们共享动作、生命周期、证据和结果格式。',
    surfaceWeb: 'Web',
    surfaceWebBody:
      '持久 Agent 会话与 ACL 套件，使用 A3S Browser 或兼容的独立浏览器。',
    surfaceGui: 'GUI',
    surfaceGuiBody:
      'macOS 上通过锁定的 A3S CUA 契约测试并完成真实主机发布认证。',
    surfaceTui: 'TUI',
    surfaceTuiBody: '通过自有 PTY / ConPTY 进程树和有界终端语义运行 ACL 套件。',
    ctaTitle: '把真实页面接入 Test Kit',
    ctaBody:
      '先在开发环境生成页面上下文，再启动 Web 会话，标记一个需要修改的问题。',
    quickStart: '阅读接入指南',
    architecture: '查看架构边界',
    footer: '观察必须新鲜，动作必须有界，结果必须有证据。',
    experience: {
      stageAria: 'A3S Test Kit 实时体验',
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
      submitted: '演示订单状态已更新。',
      contextTitle: '页面上下文',
      refresh: '刷新',
      connecting: '正在读取真实页面上下文…',
      selected: '选中元素',
      revision: '修订',
      role: '角色',
      name: '名称',
      geometry: '视口坐标',
      locator: '推荐定位器',
      source: '组件来源',
      reviewTitle: '点选问题',
      reviewBody: '点选目标，写下修改要求；可单条保存，也可整理成批次。',
      openReview: '开始标记',
      reviewStarted: '点选模式已打开',
      localOnly: '演示数据仅保存在此页，不修改代码，也不联系外部 Agent。',
      evidenceTitle: '本地任务',
      evidenceWaiting: '尚未保存任务',
      evidenceReady: '已保存到当前页面',
      findingsUnit: '个问题',
      noFinding: '尚未保存问题',
      renderedStatus: '渲染完成',
      contextStatus: '上下文已绑定',
      evidenceStatus: '本地保存',
      motionSteps: ['感知页面', '定位元素', '人工批注', '组成批次', '本地保存'],
      motionFinding: '已命中提交按钮',
      motionRequest: '提高主操作的文字对比度',
      motionPacket: '1 个问题 · 上下文已绑定',
      motionContext: '实时上下文',
      motionContextValue: 'button · 提交订单',
      motionAdd: '加入批次',
      motionSend: '保存任务',
      motionReady: '任务已在本页保存',
      motionPause: '暂停自动演示',
      motionResume: '继续自动演示',
    },
  },
  en: {
    heroTitle: ['Give agents the live page.', 'Keep every repair grounded.'],
    heroBody:
      'Test Kit produces bounded, revisioned page context after the browser renders. Mark one issue or a batch; A3S Test hands the selected target, requested change, and observed evidence to the authorized coding agent, then verifies the edited page again.',
    startExperience: 'Mark an issue in the demo',
    readDocs: 'Read the quick start',
    viewGitHub: 'View GitHub',
    installTitle: 'Install the CLI and coding-agent Skill',
    installBody:
      "Choose this machine's platform and copy one command to install matching CLI and Skill versions.",
    installTabs: 'Choose an installation platform',
    installPackage: 'CLI + Agent Skill',
    installNote:
      'The installer verifies the release archive SHA-256 and keeps the CLI and Skill on the same version.',
    installCandidateNote:
      'The next documentation version is staged; this command still pins the published stable release.',
    copy: 'Copy command',
    copied: 'Copied',
    copyError: 'Copy failed',
    proofTitle: 'Let the browser render. Then structure the facts.',
    proofBody:
      'Test Kit does not replace the browser or start by guessing from pixels. It turns DOM, accessible semantics, component source, stable locators, box-model facts, and multi-space geometry into one bounded snapshot. Old refs expire when the page changes.',
    sampleLabel: 'Sample page context',
    observationLabel: 'Observation',
    revisionLabel: 'Surface revision',
    actionLabel: 'Typed action',
    evidenceLabel: 'Durable evidence',
    pageContextLabel: 'Current target',
    pageContextValue: '@c7 button “Place order”',
    actionValue: 'click @c7 · observation 42',
    evidenceValue: 'events.jsonl + report.json',
    capabilitiesTitle: 'One traceable path from live page to verified result',
    capabilitiesBody:
      'Perception, targeting, submission, repair, and verification each have a defined input and authority. No stage can invent facts for the one before it.',
    capabilities: [
      {
        title: 'Perceive the rendered page',
        body: 'Wait for browser rendering, then read DOM, accessible semantics, interaction state, layout, and component ownership.',
        code: '01 · perceive',
      },
      {
        title: 'Bind the exact target',
        body: 'Bind role, name, stable locator, component source, and geometry to one page revision.',
        code: '02 · target',
      },
      {
        title: 'Submit with intent',
        body: 'A person confirms one target or a batch and the requested change. Only submission authorizes repair.',
        code: '03 · submit',
      },
      {
        title: 'Repair within scope',
        body: 'The coding agent addresses only the authorized issue and reports changed files and focused checks.',
        code: '04 · repair',
      },
      {
        title: 'Verify in a fresh browser',
        body: 'A3S Test captures updated page context and checks whether new evidence resolves the target issue.',
        code: '05 · verify',
      },
    ],
    workflowTitle: 'Explore unknown paths. Preserve proven paths.',
    workflowBody:
      'Use a persistent agent session while the page is still uncertain. Once the path and success condition are proven, express the same actions and evidence as ACL.',
    workflowAgent: 'Agent session: explore the current page',
    workflowAgentBody:
      'Start each turn from a fresh observation to reproduce a bug, inspect an unfamiliar page, or try a new flow.',
    workflowAcl: 'ACL suite: repeat the proven path',
    workflowAclBody:
      'Fix the important actions and assertions for local regression, CI, and stable interface contracts.',
    workflowObserve: 'Observe',
    workflowDecide: 'Decide',
    workflowAct: 'Act',
    workflowProve: 'Evidence',
    boundaryTitle: 'Submission is the repair boundary',
    boundaryBody:
      'Browser facts, optional advice, human submission, and source edits are recorded separately. Viewing advice or saving a draft never authorizes a source change. Only explicitly submitted targets and instructions enter repair.',
    boundaryFacts: 'Page facts',
    boundaryFactsBody:
      'DOM, accessible semantics, page state, geometry, and local assertions.',
    boundaryAdvice: 'Optional advice',
    boundaryAdviceBody:
      'Visual grounding and design review only return candidates with provenance and confidence.',
    boundaryHuman: 'Human submission',
    boundaryHumanBody:
      'You confirm the target, requested change, and whether to send it alone or in a batch.',
    boundaryRepair: 'Repair and verification',
    boundaryRepairBody:
      'The coding agent edits authorized source, then A3S Test verifies the page with new evidence.',
    surfacesTitle: 'Shared core contracts. Surface-owned drivers.',
    surfacesBody:
      'Web, GUI, and TUI avoid one brittle simulation layer while sharing action, lifecycle, evidence, and result contracts.',
    surfaceWeb: 'Web',
    surfaceWebBody:
      'Persistent agent sessions and ACL suites through A3S Browser or a compatible standalone browser.',
    surfaceGui: 'GUI',
    surfaceGuiBody:
      'Contract-tested on macOS through locked A3S CUA and release-certified on a real host.',
    surfaceTui: 'TUI',
    surfaceTuiBody:
      'ACL suites through owned PTY / ConPTY process trees and bounded terminal semantics.',
    ctaTitle: 'Add Test Kit to a real page',
    ctaBody:
      'Publish page context in development, start a Web session, and mark one issue you want changed.',
    quickStart: 'Read the integration guide',
    architecture: 'See architecture boundaries',
    footer: 'Fresh observations. Bounded actions. Evidence-backed results.',
    experience: {
      stageAria: 'Live A3S Test Kit experience',
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
      submitted: 'The demo order state changed.',
      contextTitle: 'Page context',
      refresh: 'Refresh',
      connecting: 'Reading the real page context…',
      selected: 'Selected element',
      revision: 'Revision',
      role: 'Role',
      name: 'Name',
      geometry: 'Viewport geometry',
      locator: 'Preferred locator',
      source: 'Component source',
      reviewTitle: 'Mark an issue',
      reviewBody:
        'Select a target and describe the change. Save one issue or collect a batch.',
      openReview: 'Start marking',
      reviewStarted: 'Marking mode is open',
      localOnly:
        'Demo data stays in this page. It does not edit source or contact an external agent.',
      evidenceTitle: 'Local task',
      evidenceWaiting: 'No task saved',
      evidenceReady: 'Saved in this page',
      findingsUnit: 'issues',
      noFinding: 'No issues saved',
      renderedStatus: 'Render complete',
      contextStatus: 'Context bound',
      evidenceStatus: 'Saved locally',
      motionSteps: [
        'Perceive page',
        'Locate element',
        'Add note',
        'Build batch',
        'Save locally',
      ],
      motionFinding: 'Submit action located',
      motionRequest: 'Increase the primary action label contrast',
      motionPacket: '1 finding · context bound',
      motionContext: 'Live context',
      motionContextValue: 'button · Place order',
      motionAdd: 'Add to batch',
      motionSend: 'Save task',
      motionReady: 'Task saved in this page',
      motionPause: 'Pause walkthrough',
      motionResume: 'Resume walkthrough',
    },
  },
};
