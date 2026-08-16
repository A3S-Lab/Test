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
  contextUnavailable: string;
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
    heroTitle: ['把页面事实交给 Agent，', '再用新证据验证修复'],
    heroBody:
      'Test Kit 在页面渲染后提取可访问语义、组件归属、稳定定位器和坐标。你点选并说明单个或一批问题，获授权的编码 Agent 按范围修改，A3S Test 再用新的页面证据验证结果。',
    startExperience: '体验页面点选',
    readDocs: '阅读快速开始',
    installTitle: '安装 CLI 和 Agent Skill',
    installBody:
      '选择平台，复制一条命令。安装器会为本机安装同版本的 CLI 与 Skill。',
    installTabs: '选择安装平台',
    installPackage: 'CLI + Agent Skill',
    installNote: '安装器校验发布归档的 SHA-256，并保持 CLI 与 Skill 版本一致。',
    installCandidateNote:
      '当前文档已进入下一版本准备阶段。此命令仍固定安装已发布的稳定版。',
    copy: '复制命令',
    copied: '命令已复制',
    copyError: '复制失败，请手动选择命令',
    proofTitle: '先分清三种事实，再比较期望与实际',
    proofBody:
      'PRD 说明产品意图，设计稿描述视觉结构，Test Kit 记录浏览器实际渲染。前两者先生成带引用的契约候选，由人审阅。A3S Test 再把获准契约与页面事实做确定性比较。',
    contractPanelLabel: '界面契约生成与核对',
    contractExpectedLabel: '期望来源',
    contractObservedLabel: '实际来源',
    contractPrdTitle: '产品意图',
    contractPrdBody: '文案、结果、约束和仍需决定的问题。',
    contractDesignTitle: '视觉期望',
    contractDesignBody: '区域层级、尺寸、位置和图像摘要。',
    contractPageTitle: '渲染事实',
    contractPageBody: '角色、名称、状态、组件、定位器和几何。',
    contractReviewTitle: '人工审阅契约',
    contractReviewBody: '确认引用、冲突和每条期望。',
    contractCompareTitle: '确定性核对',
    contractCompareBody: '比较获准期望与当前页面修订。',
    contractReportTitle: '差异报告',
    contractReportBody: '保留来源、决定和可复查证据。',
    contractDisclaimer:
      'PRD 与设计稿不会被包装成浏览器可访问树。每条期望都保留来源、审阅决定和内容摘要。',
    contractGuide: '了解界面契约',
    capabilitiesTitle: '从页面现场到验证结果，每一步都有边界',
    capabilitiesBody:
      '页面提供事实，人提交修改意图，编码 Agent 只处理授权范围，A3S Test 用新的观察判断结果。',
    capabilities: [
      {
        title: '感知真实页面',
        body: '页面渲染完成后，读取 DOM、可访问语义、交互状态、布局和组件归属。',
        code: '01 · perceive',
      },
      {
        title: '精确绑定目标',
        body: '把角色、名称、稳定定位器、组件来源和坐标绑定到同一个页面修订。',
        code: '02 · target',
      },
      {
        title: '明确提交问题',
        body: '你确认单个或一批目标与修改要求。明确提交以后，修复权限才生效。',
        code: '03 · submit',
      },
      {
        title: 'Agent 在范围内修复',
        body: '编码 Agent 只处理已授权的问题，并回报修改文件与针对性检查。',
        code: '04 · repair',
      },
      {
        title: '浏览器重新验证',
        body: 'A3S Test 获取更新后的页面上下文，用新的观察和断言确认结果。',
        code: '05 · verify',
      },
    ],
    workflowTitle: '探索时逐步决策，回归时固定执行',
    workflowBody:
      '陌生路径交给持久 Agent 会话，每个动作前先观察。成功条件确认后，把路径保存为 ACL，交给本地或 CI 重复执行。',
    workflowAgent: 'Agent 会话用于探索',
    workflowAgentBody:
      '每轮都从新观察开始，适合复现问题、检查陌生页面和试走新流程。',
    workflowAcl: 'ACL 套件用于回归',
    workflowAclBody:
      '把关键动作与断言固定下来，用于本地回归、CI 和界面契约验证。',
    workflowAgentLink: '了解 Agent 会话',
    workflowAclLink: '编写 ACL 套件',
    workflowObserve: '观察',
    workflowDecide: '决策',
    workflowAct: '动作',
    workflowProve: '证据',
    boundaryTitle: '人提交以后，修复权限才生效',
    boundaryBody:
      '浏览器事实、模型建议、人工授权和源码修改分别保存。查看建议、标记元素或保存草稿都不会改代码。只有明确发送的问题才进入修复流程。',
    boundaryFacts: '浏览器事实',
    boundaryFactsBody: 'DOM、可访问语义、页面状态、几何与本地断言。',
    boundaryAdvice: '模型候选',
    boundaryAdviceBody: '视觉定位和设计审查只提供带来源与置信度的候选。',
    boundaryHuman: '人工授权',
    boundaryHumanBody: '你确认目标、修改要求，以及要单独发送还是组成批次。',
    boundaryRepair: '修复与验证',
    boundaryRepairBody:
      '编码 Agent 修改获授权的源码，A3S Test 再用新页面证据验证。',
    surfacesTitle: '同一结果格式，三种界面各用专用驱动',
    surfacesBody:
      'Web、GUI 和 TUI 共享动作、生命周期、证据与结果契约，各自保留符合界面特性的执行方式。',
    surfaceWeb: 'Web',
    surfaceWebBody:
      '持久 Agent 会话与 ACL 套件，使用 A3S Browser 或兼容的独立浏览器。',
    surfaceGui: 'GUI',
    surfaceGuiBody:
      'macOS 使用锁定的 CUA 契约并通过真实主机发布认证。Windows 与 Linux 后端仍需单独审核。',
    surfaceTui: 'TUI',
    surfaceTuiBody: '通过自有 PTY / ConPTY 进程树和有界终端语义运行 ACL 套件。',
    ctaTitle: '先接入一个页面，再完成一次可验证修改',
    ctaBody:
      '在开发环境挂载 Test Kit，用 Agent 会话观察页面并标记一个问题。流程确认后，再把它保存为 ACL。',
    quickStart: '阅读接入指南',
    architecture: '查看架构边界',
    footer: '每次都从新观察开始。动作限制在授权范围内，结果保留证据。',
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
      contextUnavailable: '未读到页面上下文，请刷新重试。',
      selected: '选中元素',
      revision: '修订',
      role: '角色',
      name: '名称',
      geometry: '视口坐标',
      locator: '推荐定位器',
      source: '组件来源',
      reviewTitle: '点选问题',
      reviewBody:
        '点选目标并写下修改要求。你可以保存一条问题，也可以整理成批次。',
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
    heroTitle: [
      'Give agents the rendered facts.',
      'Verify changes with fresh evidence.',
    ],
    heroBody:
      'After the page renders, Test Kit captures accessible semantics, component ownership, stable locators, and geometry. Mark one issue or a batch, let an authorized coding agent edit only that scope, then verify the result with fresh page evidence.',
    startExperience: 'Try element marking',
    readDocs: 'Read the quick start',
    installTitle: 'Install the CLI and Agent Skill',
    installBody:
      'Choose a platform and copy one command. The installer keeps the local CLI and Skill on the same version.',
    installTabs: 'Choose an installation platform',
    installPackage: 'CLI + Agent Skill',
    installNote:
      'The installer verifies the release archive SHA-256 and keeps the CLI and Skill on the same version.',
    installCandidateNote:
      'The next documentation version is staged. This command still pins the published stable release.',
    copy: 'Copy command',
    copied: 'Command copied',
    copyError: 'Copy failed. Select the command manually.',
    proofTitle: 'Separate the sources. Compare expected and rendered UI.',
    proofBody:
      'A PRD states product intent, a design describes visual structure, and Test Kit records what the browser rendered. The first two produce cited contract candidates for human review. A3S Test then compares the approved contract with page facts deterministically.',
    contractPanelLabel: 'Surface contract generation and comparison',
    contractExpectedLabel: 'Expected source',
    contractObservedLabel: 'Observed source',
    contractPrdTitle: 'Product intent',
    contractPrdBody: 'Copy, outcomes, constraints, and unresolved decisions.',
    contractDesignTitle: 'Visual expectation',
    contractDesignBody: 'Regions, hierarchy, dimensions, and image digests.',
    contractPageTitle: 'Rendered fact',
    contractPageBody:
      'Roles, names, state, components, locators, and geometry.',
    contractReviewTitle: 'Human-reviewed contract',
    contractReviewBody: 'Confirm citations, conflicts, and every expectation.',
    contractCompareTitle: 'Deterministic comparison',
    contractCompareBody:
      'Compare approved expectations with this page revision.',
    contractReportTitle: 'Contract report',
    contractReportBody: 'Retain sources, decisions, and inspectable evidence.',
    contractDisclaimer:
      'PRDs and designs never masquerade as a browser accessibility tree. Every expectation retains its source, review decision, and content digest.',
    contractGuide: 'Understand surface contracts',
    capabilitiesTitle:
      'Every step from live page to verified result has a boundary',
    capabilitiesBody:
      'The page supplies facts, a person submits the change, the coding agent stays within the authorized scope, and A3S Test judges the result from a fresh observation.',
    capabilities: [
      {
        title: 'Perceive the rendered page',
        body: 'After rendering completes, read DOM, accessible semantics, interaction state, layout, and component ownership.',
        code: '01 · perceive',
      },
      {
        title: 'Bind the exact target',
        body: 'Bind role, name, stable locator, component source, and geometry to one page revision.',
        code: '02 · target',
      },
      {
        title: 'Submit with intent',
        body: 'Confirm one target or a batch and the requested change. Repair authority begins only after explicit submission.',
        code: '03 · submit',
      },
      {
        title: 'Repair within scope',
        body: 'The coding agent addresses only the authorized issue and reports changed files and focused checks.',
        code: '04 · repair',
      },
      {
        title: 'Verify in a fresh browser',
        body: 'A3S Test captures updated page context and checks the result with a fresh observation and local assertions.',
        code: '05 · verify',
      },
    ],
    workflowTitle:
      'Decide while exploring. Execute deterministically in regression.',
    workflowBody:
      'Use a persistent agent session for an unfamiliar path and observe before every action. Once the success condition is proven, save the path as ACL for local or CI repetition.',
    workflowAgent: 'Agent sessions explore',
    workflowAgentBody:
      'Start each turn from a fresh observation to reproduce a bug, inspect an unfamiliar page, or try a new flow.',
    workflowAcl: 'ACL suites regress',
    workflowAclBody:
      'Fix the important actions and assertions for local regression, CI, and stable interface contracts.',
    workflowAgentLink: 'Learn agent sessions',
    workflowAclLink: 'Write an ACL suite',
    workflowObserve: 'Observe',
    workflowDecide: 'Decide',
    workflowAct: 'Act',
    workflowProve: 'Evidence',
    boundaryTitle: 'Repair authority begins with human submission',
    boundaryBody:
      'Browser facts, model advice, human authorization, and source edits are stored separately. Viewing advice, marking an element, or saving a draft cannot change code. Only explicitly sent issues enter repair.',
    boundaryFacts: 'Browser facts',
    boundaryFactsBody:
      'DOM, accessible semantics, page state, geometry, and local assertions.',
    boundaryAdvice: 'Model candidates',
    boundaryAdviceBody:
      'Visual grounding and design review only return candidates with provenance and confidence.',
    boundaryHuman: 'Human authorization',
    boundaryHumanBody:
      'You confirm the target, requested change, and whether to send it alone or in a batch.',
    boundaryRepair: 'Repair and verification',
    boundaryRepairBody:
      'The coding agent edits authorized source, then A3S Test verifies the page with new evidence.',
    surfacesTitle: 'One result contract, one purpose-built driver per surface',
    surfacesBody:
      'Web, GUI, and TUI share action, lifecycle, evidence, and result contracts while retaining execution designed for each interface.',
    surfaceWeb: 'Web',
    surfaceWebBody:
      'Persistent agent sessions and ACL suites through A3S Browser or a compatible standalone browser.',
    surfaceGui: 'GUI',
    surfaceGuiBody:
      'macOS uses a locked CUA contract and real-host release certification. Windows and Linux backends still require separate review.',
    surfaceTui: 'TUI',
    surfaceTuiBody:
      'ACL suites through owned PTY / ConPTY process trees and bounded terminal semantics.',
    ctaTitle: 'Connect one page and complete one verifiable change',
    ctaBody:
      'Mount Test Kit in development, observe the page through an agent session, and mark one issue. Save the proven flow as ACL when it is stable.',
    quickStart: 'Read the integration guide',
    architecture: 'See architecture boundaries',
    footer: 'Fresh observations. Scoped actions. Inspectable evidence.',
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
      contextUnavailable: 'Page context is unavailable. Refresh to try again.',
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
