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
    heroTitle: ['让 Agent 看懂页面，', '点中问题，交给它修'],
    heroBody:
      '接入 Test Kit 后，A3S Test 会读取页面当前渲染出的 DOM、可访问语义、组件来源和坐标。你在页面上点选一个或一批问题，修复任务会连同现场上下文交给编码 Agent。',
    startExperience: '体验点选修复',
    readDocs: '5 分钟快速开始',
    viewGitHub: '查看 GitHub',
    installTitle: '一条命令装好 CLI 和 Skill',
    installBody:
      '自动识别 macOS、Linux 或 Windows，并为本机编码 Agent 安装匹配版本。',
    installTabs: '选择安装平台',
    installPackage: 'CLI + Agent Skill',
    installNote: '安装器校验发布归档的 SHA-256，并保持 CLI 与 Skill 版本一致。',
    copy: '复制命令',
    copied: '已复制',
    copyError: '复制失败',
    proofTitle: '页面一渲染，测试上下文就准备好了',
    proofBody:
      'A3S Test 读取浏览器当前看到的结构和状态，同时补上组件归属、稳定定位器与多坐标空间几何。页面变化后，旧引用立即失效。',
    sampleLabel: '页面上下文示例',
    observationLabel: '观察',
    revisionLabel: '界面修订',
    actionLabel: '类型化动作',
    evidenceLabel: '持久证据',
    pageContextLabel: '当前目标',
    pageContextValue: '@c7 button “Place order”',
    actionValue: 'click @c7 · observation 42',
    evidenceValue: 'events.jsonl + report.json',
    capabilitiesTitle: '从发现问题，到交付可验证的修复',
    capabilitiesBody:
      '你在页面上指出问题。A3S Test 保存目标、说明和现场证据。编码 Agent 修改源码后，再用新的浏览器会话验证结果。',
    capabilities: [
      {
        title: '读懂当前页面',
        body: '从浏览器语义和 Test Kit 上下文读取页面结构、状态与组件归属。',
        code: 'observe → context → act',
      },
      {
        title: '准确定位元素',
        body: '优先使用角色、名称和稳定定位器，必要时再结合当前视口坐标。',
        code: 'role + name + geometry',
      },
      {
        title: '直接点选并批量提交',
        body: '在页面上点击或框选问题，写下修改要求，再单条发送或组成一个批次。',
        code: 'mark → review → send',
      },
      {
        title: '把成功路径变成回归测试',
        body: '探索流程跑通后保存为 ACL，后续在本地或 CI 中重复验证。',
        code: 'check → run → report',
      },
    ],
    workflowTitle: '探索一次，留下可重复的回归测试',
    workflowBody:
      '第一次由编码 Agent 根据页面现状逐步探索。流程确认后，把关键动作和断言保存成 ACL，交给本地开发或 CI 反复执行。',
    workflowAgent: '先让 Agent 走通真实流程',
    workflowAgentBody: '适合复现问题、检查陌生页面和确认一条新流程能否完成。',
    workflowAcl: '再把路径固定成 ACL',
    workflowAclBody: '适合回归测试、CI 和需要稳定结果的界面契约验证。',
    workflowObserve: '观察',
    workflowDecide: '决策',
    workflowAct: '动作',
    workflowProve: '证明',
    boundaryTitle: '人决定修什么，Agent 负责改代码',
    boundaryBody:
      '页面事实、模型建议、人工决定和源码修改分别记录。打开建议不会触发改动，只有明确提交的标记才会进入修复。',
    boundaryFacts: '页面事实',
    boundaryFactsBody: '浏览器观察、Test Kit 上下文与本地断言。',
    boundaryAdvice: '模型建议',
    boundaryAdviceBody: '定位候选和设计审查带着来源与置信度进入评审。',
    boundaryHuman: '人工决定',
    boundaryHumanBody: '你确认目标、修改要求和需要发送的批次。',
    boundaryRepair: '代码修改',
    boundaryRepairBody: '编码 Agent 修改源码，A3S Test 再打开页面验证。',
    surfacesTitle: '同一套结果，覆盖 Web、桌面和终端',
    surfacesBody: '不同界面使用各自的驱动，动作、证据和测试结果保持一致。',
    surfaceWeb: 'Web',
    surfaceWebBody:
      '持久 Agent 会话与 ACL 套件，使用 A3S Browser 或兼容的独立浏览器。',
    surfaceGui: 'GUI',
    surfaceGuiBody:
      'macOS 上通过锁定的 A3S CUA 契约测试并完成真实主机发布认证。',
    surfaceTui: 'TUI',
    surfaceTuiBody: '通过自有 PTY / ConPTY 进程树和有界终端语义运行 ACL 套件。',
    ctaTitle: '从一个真实页面开始',
    ctaBody: '接入 Test Kit，启动一次 Web 会话，亲手点选一个需要修改的问题。',
    quickStart: '开始接入',
    architecture: '查看架构边界',
    footer: '让页面可理解，让修复可验证。',
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
      reviewBody: '打开标记层，点击页面里的任意元素，写下你希望怎么改。',
      openReview: '打开点选模式',
      reviewStarted: '点选模式已打开',
      localOnly:
        '这个演示只保存在当前页面，不会修改源码，也不会发送给外部 Agent。',
      evidenceTitle: '修复任务',
      evidenceWaiting: '等待你提交',
      evidenceReady: '已保存在当前页面',
      findingsUnit: '个标记',
      noFinding: '还没有提交标记',
      renderedStatus: '页面已渲染',
      contextStatus: '上下文已读取',
      evidenceStatus: '任务已保存',
      motionSteps: ['扫描页面', '命中元素', '读取语义', '人工批注', '批量发送'],
      motionFinding: '已命中提交按钮',
      motionRequest: '提高主操作的文字对比度',
      motionPacket: '1 个问题 · 上下文已绑定',
    },
  },
  en: {
    heroTitle: ['Agents understand the page.', 'You point. They repair.'],
    heroBody:
      'Add Test Kit and A3S Test reads the rendered DOM, accessible semantics, component source, and geometry. Mark one issue or a batch on the page, then send the task with its live context to your coding agent.',
    startExperience: 'Try point-and-repair',
    readDocs: '5-minute quick start',
    viewGitHub: 'View GitHub',
    installTitle: 'Install the CLI and Skill in one command',
    installBody:
      'Detect macOS, Linux, or Windows and install the matching version for local coding agents.',
    installTabs: 'Choose an installation platform',
    installPackage: 'CLI + Agent Skill',
    installNote:
      'The installer verifies the release archive SHA-256 and keeps the CLI and Skill on the same version.',
    copy: 'Copy command',
    copied: 'Copied',
    copyError: 'Copy failed',
    proofTitle: 'Page context is ready as soon as the UI renders',
    proofBody:
      'A3S Test reads the structure and state visible to the browser, then adds component ownership, stable locators, and geometry across coordinate spaces. Old targets expire when the page changes.',
    sampleLabel: 'Sample page context',
    observationLabel: 'Observation',
    revisionLabel: 'Surface revision',
    actionLabel: 'Typed action',
    evidenceLabel: 'Durable evidence',
    pageContextLabel: 'Current target',
    pageContextValue: '@c7 button “Place order”',
    actionValue: 'click @c7 · observation 42',
    evidenceValue: 'events.jsonl + report.json',
    capabilitiesTitle: 'From a visible problem to a verified repair',
    capabilitiesBody:
      'You point to the issue. A3S Test keeps the target, instruction, and page evidence together. After the coding agent edits the source, a fresh browser session verifies the result.',
    capabilities: [
      {
        title: 'Understand the current page',
        body: 'Read page structure, state, and component ownership from browser semantics and Test Kit context.',
        code: 'observe → context → act',
      },
      {
        title: 'Target the right element',
        body: 'Prefer roles, names, and stable locators, with current viewport geometry available when needed.',
        code: 'role + name + geometry',
      },
      {
        title: 'Mark one issue or a batch',
        body: 'Click or draw around the problem, describe the change, then send it alone or with related findings.',
        code: 'mark → review → send',
      },
      {
        title: 'Keep successful paths as regressions',
        body: 'Once an exploratory flow works, preserve it as ACL and rerun it locally or in CI.',
        code: 'check → run → report',
      },
    ],
    workflowTitle: 'Explore once. Keep a repeatable regression.',
    workflowBody:
      'A coding agent explores the page step by step the first time. When the flow is confirmed, save the important actions and assertions as ACL for local development or CI.',
    workflowAgent: 'Let the agent prove the real flow first',
    workflowAgentBody:
      'Use it to reproduce a bug, inspect an unfamiliar page, or confirm a new journey.',
    workflowAcl: 'Then preserve the path as ACL',
    workflowAclBody:
      'Use it for regression tests, CI, and interface contracts that need stable results.',
    workflowObserve: 'Observe',
    workflowDecide: 'Decide',
    workflowAct: 'Act',
    workflowProve: 'Prove',
    boundaryTitle: 'You choose the repair. The agent edits the code.',
    boundaryBody:
      'Page facts, model suggestions, human decisions, and source edits are recorded separately. Viewing a suggestion never changes code. Only submitted findings enter repair.',
    boundaryFacts: 'Page facts',
    boundaryFactsBody:
      'Browser observations, Test Kit context, and local assertions.',
    boundaryAdvice: 'Model suggestions',
    boundaryAdviceBody:
      'Grounding and design-review candidates retain provenance and confidence.',
    boundaryHuman: 'Human decision',
    boundaryHumanBody:
      'You confirm the target, requested change, and findings to send.',
    boundaryRepair: 'Source edit',
    boundaryRepairBody:
      'The coding agent edits source, then A3S Test verifies the page in a fresh browser.',
    surfacesTitle: 'One result format across Web, desktop, and terminal',
    surfacesBody:
      'Each surface uses its own driver while actions, evidence, and results stay consistent.',
    surfaceWeb: 'Web',
    surfaceWebBody:
      'Persistent agent sessions and ACL suites through A3S Browser or a compatible standalone browser.',
    surfaceGui: 'GUI',
    surfaceGuiBody:
      'Contract-tested on macOS through locked A3S CUA and release-certified on a real host.',
    surfaceTui: 'TUI',
    surfaceTuiBody:
      'ACL suites through owned PTY / ConPTY process trees and bounded terminal semantics.',
    ctaTitle: 'Start with one real page',
    ctaBody:
      'Add Test Kit, start a Web session, and mark one issue you want changed.',
    quickStart: 'Start integrating',
    architecture: 'See architecture boundaries',
    footer: 'Make the page understandable. Make the repair verifiable.',
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
        'Open the marking layer, click any element on the page, and describe what should change.',
      openReview: 'Open marking mode',
      reviewStarted: 'Marking mode is open',
      localOnly:
        'This demo stays in the current page. It does not edit source or contact an external agent.',
      evidenceTitle: 'Repair task',
      evidenceWaiting: 'Waiting for your submission',
      evidenceReady: 'Saved in this page',
      findingsUnit: 'findings',
      noFinding: 'No findings submitted yet',
      renderedStatus: 'Page rendered',
      contextStatus: 'Context ready',
      evidenceStatus: 'Task saved',
      motionSteps: [
        'Scan page',
        'Locate element',
        'Read semantics',
        'Add note',
        'Send batch',
      ],
      motionFinding: 'Submit action located',
      motionRequest: 'Increase the primary action label contrast',
      motionPacket: '1 finding · context bound',
    },
  },
};
