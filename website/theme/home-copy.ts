import type { ExperienceCopy, Locale, LocalizedCopy } from './home-copy-types';
import { semanticStateCapabilities } from './semantic-state-capabilities';

export type {
  CapabilityGroupId,
  ExperienceCopy,
  Locale,
} from './home-copy-types';

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
    capabilityLedgerTitle: '功能清单细到输入、证据和失效规则',
    capabilityLedgerBody:
      '展开任一组即可核对采集信号、公开引用、权限边界和验证方式。这里列的是当前实现契约，不把可选模型或待审核平台写成既有能力。',
    capabilityReference: '查看 47 项能力的入口、输出、证据和失败边界',
    capabilityItemCount: '项可核验能力',
    capabilityGroups: [
      {
        id: 'context',
        code: 'PAGE CONTEXT',
        title: '页面感知',
        summary:
          '从浏览器已渲染结果中提取语义、组件、定位器、几何、布局和动效证据。',
        href: '/concepts/page-context.html',
        linkLabel: '查看 Page Context 字段、预算与失效规则',
        items: [
          {
            signal: 'DOM · AX · FORM',
            title: '语义结构与表单状态',
            body: '遍历 Light DOM 与开放 Shadow DOM，保留角色、可访问名称、原生交互状态和经过脱敏的表单状态。',
          },
          {
            signal: 'BOUNDARY · SOURCE',
            title: '组件归属与源码提示',
            body: '显式边界可补充组件 ID、名称、多根盒子、ready、受控 facts 与文件行列提示；没有边界时自动 DOM 上下文仍可工作。',
          },
          {
            signal: 'ROLE → CSS',
            title: '稳定定位器候选链',
            body: '按角色与名称、label、test ID、placeholder、文本和 CSS 的顺序保留候选，屏幕坐标只作为最后手段。',
          },
          {
            signal: 'VIEWPORT · DOCUMENT · NORMALIZED',
            title: '三套坐标与可见性',
            body: '记录视口、文档和 visual viewport 标准化几何，同时保留可见比例、遮挡、定位方式、变换和最近滚动容器。',
          },
          {
            signal: 'FLEX · GRID · BOX',
            title: '布局图与视觉系统',
            body: '读取样式令牌、Flex、Grid、普通流、层叠关系、精确 client 与 scroll 尺寸、逐轴裁剪和物理盒模型边值。',
          },
          {
            signal: 'STATE · TIMELINE',
            title: '真实状态差分与动效',
            body: '只记录页面自然出现的 hover、focus、checked 等差分，并描述 transition、CSS 与 Web Animations、时间轴、范围和 reduced motion。',
          },
          {
            signal: 'REVISION · BUDGET',
            title: '修订、范围与预算',
            body: 'DOM、尺寸、路由、视口和滚动变化推进修订；支持页面、节点、组件与区域范围，以及 summary、scoped、diff、forensic 明细和有界分页。',
          },
        ],
      },
      {
        id: 'safety',
        code: 'ACTION SAFETY',
        title: '动作与安全',
        summary:
          '每个动作都要通过最新观察、类型 Schema、能力策略、来源和运行时所有权检查。',
        href: '/concepts/authority-and-safety.html',
        linkLabel: '查看权限、安全和失败关闭规则',
        items: [
          {
            signal: '@eN · @cN · @uN',
            title: '观察绑定引用',
            body: '@eN 与 @cN 只在生成它们的最新观察中可操作。@uN 永远只读，用于连接样式、布局、状态和动效证据。',
          },
          {
            signal: 'SCHEMA · POLICY',
            title: '类型化动作准入',
            body: '点击、填写、拖动、等待和证据动作先校验字段、会话能力、目标类型与策略，再交给界面驱动执行。',
          },
          {
            signal: 'ORIGIN · NETWORK',
            title: '来源与网络边界',
            body: 'URL 动作和观察使用精确 origin 门禁，网络访问使用单独的允许范围；页面跳离已授权来源后不会继续签发新引用。',
          },
          {
            signal: 'REDACT · UNTRUSTED',
            title: '脱敏与不可信上下文',
            body: '密码、Cookie、存储、请求头和密钥不会进入 Page Context。页面文本、facts 和修复说明始终作为不可信证据处理。',
          },
          {
            signal: 'PROCESS · ARTIFACT',
            title: '精确进程与证据所有权',
            body: '每次运行只清理自己创建的浏览器或进程树，证据路径受限在本次会话目录，不按进程名关闭开发者已有会话。',
          },
        ],
      },
      {
        id: 'repair',
        code: 'HUMAN REVIEW',
        title: '人工评审与修复',
        summary:
          '评审者先标记和组织问题，再明确发送；拥有工作区的编码 Agent 才能进入修复与验证。',
        href: '/guide/repairs.html',
        linkLabel: '查看单项、批量、队列与验收流程',
        items: [
          {
            signal: 'ELEMENT · TEXT · MULTI · REGION · DRAW',
            title: '五种页面标记',
            body: '支持元素、选中文本、有序多选、矩形区域和自由手绘，并把目标绑定到发送时的最新页面修订。',
          },
          {
            signal: 'PLACEMENT · REARRANGE',
            title: '类型化布局意图',
            body: 'Layout Mode 记录新增组件区域或现有区块目标位置，不移动节点、不写内联样式，也不把 overlay 变成页面编辑器。',
          },
          {
            signal: 'MEMORY · SESSION · LOCAL',
            title: '草稿与本地存储',
            body: '草稿可留在内存、当前标签页或本地浏览器。打开编辑器、查看建议和保存草稿都不会授予源码修改权限。',
          },
          {
            signal: 'SINGLE · BATCH · CONFLICT',
            title: '单项、批量与显式冲突',
            body: '单项发送和批量发送共享稳定顺序。互斥需求使用 conflicts_with 关系声明，系统不会从自然语言猜测冲突。',
          },
          {
            signal: 'BRIDGE · SAME ORIGIN',
            title: '显式发送通道',
            body: '问题可由浏览器会话从 bridge 队列提取，也可 POST 到可选同源端点。端点只转发有界记录，不接收工作区或 Agent 凭据。',
          },
          {
            signal: 'QUEUE → REVIEW',
            title: '追加式修复状态',
            body: 'queued、claimed、repairing、verifying 和 review_ready 等变化带序号、操作者与 attempt ID，非法跳转不会改写状态。',
          },
          {
            signal: 'BEFORE · AFTER',
            title: '新页面验证与人工验收',
            body: '验证要求更新后的 ready 修订，重新定位目标并比较断言、截图、console 和 page errors。默认由人接受、拒绝或重新打开结果。',
          },
        ],
      },
      {
        id: 'execution',
        code: 'SURFACE EXECUTION',
        title: '界面执行',
        summary:
          '把导航、指针、表单、键盘、浏览器上下文、同步和文件操作表达成类型化动作。',
        href: '/reference/capabilities.html#界面执行',
        linkLabel: '查看动作入口、目标要求和失败行为',
        items: [
          {
            signal: 'NAVIGATE · SNAPSHOT · VIEWPORT',
            title: '导航、观察与视口',
            body: '导航受精确 origin 约束；snapshot 生成新观察；viewport 用明确宽高与缩放重建响应式条件，并使旧引用失效。',
          },
          {
            signal: 'CLICK · FILL · SELECT · DRAG',
            title: '指针与表单动作',
            body: '支持点击、悬停、聚焦、双击、右键、填写、追加输入、勾选、选择与拖动，目标必须匹配动作需要的角色和状态。',
          },
          {
            signal: 'PRESS · INSERT_TEXT · WHEEL',
            title: '键盘、编辑上下文与滚轮',
            body: '按键与修饰键滚轮使用类型化参数；insert_text 只复用已经建立的焦点或选择范围，不获得新的目标权限。',
          },
          {
            signal: 'TAB · FRAME · DIALOG',
            title: '标签页、Frame 与对话框',
            body: '切换浏览器上下文后必须重新观察。对话框接受与拒绝显式建模，跨域 Frame 仍受浏览器与来源策略限制。',
          },
          {
            signal: 'WAIT · ASSERT',
            title: '有界等待与本地断言',
            body: '按 load、文本、URL 或可见性等待和断言，全部受场景或命令 deadline 约束，不用无限 sleep 推测页面已经完成。',
          },
          ...semanticStateCapabilities.zh,
          {
            signal: 'FOCUSED · UNFOCUSED',
            title: '精确焦点归属',
            body: '把稳定目标与当前文档及开放 Shadow DOM 中最深的 activeElement 原子比较；目标缺失不能证明未聚焦。',
          },
          {
            signal: 'FOCUS_WITHIN · FOCUS_OUTSIDE',
            title: '组件内焦点归属',
            body: '沿 assigned slot、DOM 父级与 Shadow host 验证焦点是否属于组件范围，可回归焦点陷阱、恢复和真实 Tab 顺序。',
          },
          {
            signal: 'IN_VIEWPORT · VISUAL VIEWPORT',
            title: '视口正面积相交',
            body: '从浏览器原子采集目标和 visual viewport 矩形，在 Rust 中重算相交比例；离屏或只有边界接触不会被可见边界盒误判为在视口内。',
          },
          {
            signal: 'POINTER_REACHABLE · 3×3 HIT TEST',
            title: '确定性指针命中可达',
            body: '在可见交集上验证固定九点深层命中，可识别遮挡、pointer-events、子元素和开放 Shadow DOM，同时不冒充 enabled 或业务可点击性。',
          },
          {
            signal: 'UPLOAD · DOWNLOAD · ROUTE',
            title: '文件与网络控制',
            body: '上传、下载和网络路由都经过清单、路径与域名准入；下载和网络证据只能写入本次运行拥有的目录。',
          },
          {
            signal: 'WEB · GUI · TUI',
            title: '跨界面动作契约',
            body: 'Core 复用动作、策略、结果和清理语义，各驱动独立实现感知与执行。Web、macOS CUA 和 TUI 的验证范围分别披露。',
          },
        ],
      },
      {
        id: 'contracts',
        code: 'EXPECTATIONS & MODELS',
        title: '期望、契约与模型建议',
        summary:
          '把 PRD、设计稿和可选模型输出保留为带来源候选，经人工审阅后再与真实页面核对。',
        href: '/guide/contracts.html',
        linkLabel: '查看候选生成、人工审阅和页面对账',
        items: [
          {
            signal: 'PRD · BYTE RANGE · DIGEST',
            title: 'PRD 候选生成',
            body: '把用户结果、文案和业务约束绑定到原文件摘要与精确字节范围，超出来源、预算或置信边界的候选会被拒绝。',
          },
          {
            signal: 'DESIGN · REGION · HIERARCHY',
            title: '设计稿候选生成',
            body: '把区域、层级、几何和视觉关系绑定到图像摘要与像素区域，不伪造浏览器角色、名称或交互状态。',
          },
          {
            signal: 'DRAFT · REVIEW · ACL',
            title: '人工审阅与发布',
            body: '生成结果保持 candidate-only。评审文件逐项批准、拒绝或解决冲突，来源重新校验后才输出规范 Surface Contract。',
          },
          {
            signal: 'VERIFY_CONTRACT · PROVENANCE',
            title: '契约与页面确定性对账',
            body: '按 test ID、组件、角色与名称匹配期望和当前页面。阻断差异影响套件，建议差异只进入带来源报告。',
          },
          {
            signal: 'GROUND · PNG SHA-256',
            title: '可选视觉定位',
            body: '显式请求或确定性定位失败时，可调用部署方 provider 返回点或矩形候选；截图摘要、观察和预算不匹配就关闭失败。',
          },
          {
            signal: 'AUDIT · FORENSIC · ADVISORY',
            title: '可选设计审查',
            body: '截图与 forensic Page Context 共同绑定层级、布局、排版、色彩和响应式建议。报告不含 verdict、动作或修复授权。',
          },
        ],
      },
      {
        id: 'evidence',
        code: 'EVIDENCE & ACL',
        title: '证据、回归与调度',
        summary:
          '把观察、动作、断言和工件保留为可审计记录，再把稳定路径放进本地、CI 或分布式运行。',
        href: '/guide/workflows.html',
        linkLabel: '查看 Agent 会话和 ACL 回归工作流',
        items: [
          {
            signal: 'PNG · AX · CONSOLE · ERRORS',
            title: '默认小证据集',
            body: '按需保存截图、可访问树、console 和 page errors；HAR、trace 与视频只在确有诊断价值的时间窗口开启。',
          },
          {
            signal: 'HAR · TRACE · VIDEO · DOWNLOAD',
            title: '按需诊断工件',
            body: 'HAR、Chrome trace、WebM 视频和受约束下载按明确窗口启停，避免把整段会话变成体积巨大且难审阅的证据包。',
          },
          {
            signal: 'SESSION · EVENTS · REPORT',
            title: '持久 Agent 会话',
            body: '会话保留目标、观察、类型化动作、追加式事件、证据和终态报告，编码 Agent 每次依据最新页面决定一个动作。',
          },
          {
            signal: 'CHECK · RUN · CI',
            title: '确定性 ACL 套件',
            body: '跑通的最小路径可写成 ACL，先静态准入，再在本地与 CI 重复动作、等待、断言和有界清理。',
          },
          {
            signal: 'INVENTORY · SHARD · REMOTE',
            title: '能力清单与分布式分片',
            body: 'Worker 先报告平台、驱动、协议和容量，再接收已准入分片。远程请求不能选择可执行文件、凭据或宿主网络策略。',
          },
          {
            signal: 'PRODUCT · SPEC · INFRA',
            title: '可区分的失败结果',
            body: '报告区分产品断言失败、测试规范错误、驱动或环境故障和清理失败，让下一步修改有明确归属。',
          },
        ],
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
    capabilityLedgerTitle:
      'Inspect capabilities down to signals, evidence, and expiry rules',
    capabilityLedgerBody:
      'Expand a group to inspect collection signals, public refs, authority boundaries, and verification. This ledger describes implemented contracts without presenting optional models or unverified platforms as built-in support.',
    capabilityReference:
      'Inspect all 47 entry points, outputs, evidence rules, and failure boundaries',
    capabilityItemCount: 'verifiable capabilities',
    capabilityGroups: [
      {
        id: 'context',
        code: 'PAGE CONTEXT',
        title: 'Page perception',
        summary:
          'Derive semantics, components, locators, geometry, layout, and motion evidence from the rendered browser state.',
        href: '/concepts/page-context.html',
        linkLabel: 'Inspect Page Context fields, budgets, and expiry',
        items: [
          {
            signal: 'DOM · AX · FORM',
            title: 'Semantic structure and form state',
            body: 'Traverse light DOM and open Shadow DOM while preserving roles, accessible names, native interaction state, and redacted form state.',
          },
          {
            signal: 'BOUNDARY · SOURCE',
            title: 'Component ownership and source hints',
            body: 'Explicit boundaries add component ID, name, multi-root boxes, readiness, controlled facts, and optional file locations. Automatic DOM context still works without them.',
          },
          {
            signal: 'ROLE → CSS',
            title: 'Stable locator candidates',
            body: 'Retain role and name, label, test ID, placeholder, text, and CSS candidates in order. Screen coordinates remain a last resort.',
          },
          {
            signal: 'VIEWPORT · DOCUMENT · NORMALIZED',
            title: 'Three coordinate spaces and visibility',
            body: 'Record viewport, document, and visual-viewport-normalized geometry with visible ratio, occlusion, positioning, transforms, and the nearest scroll container.',
          },
          {
            signal: 'FLEX · GRID · BOX',
            title: 'Layout graph and visual system',
            body: 'Read style tokens, Flex, Grid, normal flow, stacking, exact client and scroll extents, per-axis clipping, and resolved physical box-model edges.',
          },
          {
            signal: 'STATE · TIMELINE',
            title: 'Observed state differences and motion',
            body: 'Keep naturally occurring hover, focus, checked, and related differences plus transitions, CSS and Web Animations, timelines, ranges, and reduced motion.',
          },
          {
            signal: 'REVISION · BUDGET',
            title: 'Revisions, scopes, and budgets',
            body: 'DOM, size, route, viewport, and scroll changes advance the revision. Page, node, component, and region scopes support summary, scoped, diff, forensic, and bounded pagination.',
          },
        ],
      },
      {
        id: 'safety',
        code: 'ACTION SAFETY',
        title: 'Actions and safety',
        summary:
          'Admit every action through the latest observation, typed schema, capability policy, origin, and runtime ownership checks.',
        href: '/concepts/authority-and-safety.html',
        linkLabel: 'Review authority, safety, and fail-closed rules',
        items: [
          {
            signal: '@eN · @cN · @uN',
            title: 'Observation-bound refs',
            body: '@eN and @cN are actionable only in the latest observation that created them. @uN is permanently read-only UI evidence.',
          },
          {
            signal: 'SCHEMA · POLICY',
            title: 'Typed action admission',
            body: 'Clicks, fills, drags, waits, and evidence actions validate fields, session capability, target type, and policy before reaching a surface driver.',
          },
          {
            signal: 'ORIGIN · NETWORK',
            title: 'Origin and network boundaries',
            body: 'URL actions and observations use exact-origin gates while network access has a separate allowlist. A page that leaves its admitted origin receives no new refs.',
          },
          {
            signal: 'REDACT · UNTRUSTED',
            title: 'Redaction and untrusted context',
            body: 'Passwords, cookies, storage, request headers, and secrets never enter Page Context. Page text, facts, and repair instructions remain untrusted evidence.',
          },
          {
            signal: 'PROCESS · ARTIFACT',
            title: 'Exact process and artifact ownership',
            body: 'Each run cleans up only the browser or process tree it created. Evidence stays inside its session root, and unrelated developer sessions are never killed by name.',
          },
        ],
      },
      {
        id: 'repair',
        code: 'HUMAN REVIEW',
        title: 'Human review and repair',
        summary:
          'A reviewer marks and organizes findings before explicit submission. Only a workspace-owning coding agent can enter repair and verification.',
        href: '/guide/repairs.html',
        linkLabel: 'Inspect single, batch, queue, and acceptance flows',
        items: [
          {
            signal: 'ELEMENT · TEXT · MULTI · REGION · DRAW',
            title: 'Five page-marking modes',
            body: 'Mark elements, selected text, ordered multi-selection, rectangles, or freehand regions, then bind the target to the fresh page revision at submission.',
          },
          {
            signal: 'PLACEMENT · REARRANGE',
            title: 'Typed layout intent',
            body: 'Layout Mode records a new component region or a destination for an existing section without moving nodes, writing inline styles, or becoming a page builder.',
          },
          {
            signal: 'MEMORY · SESSION · LOCAL',
            title: 'Draft and browser storage',
            body: 'Keep drafts in memory, the current tab, or local browser storage. Opening an editor, viewing advice, and saving a draft never authorize source edits.',
          },
          {
            signal: 'SINGLE · BATCH · CONFLICT',
            title: 'Single, batch, and explicit conflicts',
            body: 'Single and batch submission preserve stable visible order. Incompatible requests use a typed conflicts_with relation rather than inferred language conflict.',
          },
          {
            signal: 'BRIDGE · SAME ORIGIN',
            title: 'Explicit submission channels',
            body: 'A browser session can drain the bridge queue, or the page can POST to an optional same-origin adapter. That adapter receives bounded records and no workspace credentials.',
          },
          {
            signal: 'QUEUE → REVIEW',
            title: 'Append-only repair state',
            body: 'Queued, claimed, repairing, verifying, and review-ready transitions carry sequence, actor, and attempt identity. Invalid transitions do not mutate state.',
          },
          {
            signal: 'BEFORE · AFTER',
            title: 'Fresh verification and human acceptance',
            body: 'Verification requires a newer ready revision, re-resolves the target, and compares assertions, screenshots, console, and page errors. A person accepts, rejects, or reopens by default.',
          },
        ],
      },
      {
        id: 'execution',
        code: 'SURFACE EXECUTION',
        title: 'Surface execution',
        summary:
          'Express navigation, pointer, form, keyboard, browser-context, synchronization, and file operations as typed actions.',
        href: '/reference/capabilities.html#surface-execution',
        linkLabel:
          'Inspect action entry points, target requirements, and failures',
        items: [
          {
            signal: 'NAVIGATE · SNAPSHOT · VIEWPORT',
            title: 'Navigation, observation, and viewport',
            body: 'Navigation is exact-origin scoped, snapshot creates a fresh observation, and viewport re-evaluates responsive conditions with explicit dimensions and scale while expiring old refs.',
          },
          {
            signal: 'CLICK · FILL · SELECT · DRAG',
            title: 'Pointer and form actions',
            body: 'Click, hover, focus, double-click, context-click, fill, type, check, select, and drag require a target whose role and state fit the operation.',
          },
          {
            signal: 'PRESS · INSERT_TEXT · WHEEL',
            title: 'Keyboard, edit context, and wheel',
            body: 'Keys and modified wheel gestures use typed parameters. insert_text reuses an established focus or selection and grants no new targeting authority.',
          },
          {
            signal: 'TAB · FRAME · DIALOG',
            title: 'Tabs, frames, and dialogs',
            body: 'A browser-context switch requires a fresh observation. Dialog acceptance is explicit, while cross-origin frames remain constrained by browser and origin policy.',
          },
          {
            signal: 'WAIT · ASSERT',
            title: 'Bounded waits and local assertions',
            body: 'Wait or assert on load, text, URL, or visibility under scenario and command deadlines instead of inferring readiness from an unbounded sleep.',
          },
          ...semanticStateCapabilities.en,
          {
            signal: 'FOCUSED · UNFOCUSED',
            title: 'Exact focus ownership',
            body: 'Atomically compare a stable target with the deepest activeElement across the current document and open Shadow DOM. A missing target never proves unfocused state.',
          },
          {
            signal: 'FOCUS_WITHIN · FOCUS_OUTSIDE',
            title: 'Component-scoped focus ownership',
            body: 'Follow assigned slots, DOM parents, and shadow hosts to verify component ownership for focus traps, restoration, and real Tab order.',
          },
          {
            signal: 'IN_VIEWPORT · VISUAL VIEWPORT',
            title: 'Positive-area viewport intersection',
            body: 'Capture target and visual-viewport rectangles atomically and recompute their ratio in Rust. Offscreen or boundary-only contact cannot masquerade as in-viewport merely because a rendered box exists.',
          },
          {
            signal: 'POINTER_REACHABLE · 3×3 HIT TEST',
            title: 'Deterministic pointer hit reachability',
            body: 'Validate a fixed nine-point deep-hit grid over the visible intersection to expose occlusion, pointer-events, child hits, and open Shadow DOM without claiming enabled state or business clickability.',
          },
          {
            signal: 'UPLOAD · DOWNLOAD · ROUTE',
            title: 'File and network controls',
            body: 'Uploads, downloads, and network routes pass manifest, path, and domain admission. Downloads and network evidence stay within run-owned directories.',
          },
          {
            signal: 'WEB · GUI · TUI',
            title: 'Cross-surface action contract',
            body: 'Core shares action, policy, result, and cleanup semantics while each driver owns perception and execution. Web, macOS CUA, and TUI verification scopes are disclosed separately.',
          },
        ],
      },
      {
        id: 'contracts',
        code: 'EXPECTATIONS & MODELS',
        title: 'Expectations, contracts, and model advice',
        summary:
          'Keep PRD, design, and optional model output as source-bound candidates, then compare reviewed expectations with the rendered page.',
        href: '/guide/contracts.html',
        linkLabel:
          'Inspect candidate generation, human review, and page comparison',
        items: [
          {
            signal: 'PRD · BYTE RANGE · DIGEST',
            title: 'PRD candidate generation',
            body: 'Bind user outcomes, copy, and business constraints to a source digest and exact byte range. Reject candidates outside provenance, budget, or confidence bounds.',
          },
          {
            signal: 'DESIGN · REGION · HIERARCHY',
            title: 'Design candidate generation',
            body: 'Bind regions, hierarchy, geometry, and visual relationships to an image digest and pixel region without inventing browser roles, names, or interaction state.',
          },
          {
            signal: 'DRAFT · REVIEW · ACL',
            title: 'Human review and publication',
            body: 'Generated output remains candidate-only. A review approves, rejects, or resolves each conflict before source revalidation emits a canonical Surface Contract.',
          },
          {
            signal: 'VERIFY_CONTRACT · PROVENANCE',
            title: 'Deterministic contract comparison',
            body: 'Match expectations to the current page by test ID, component, role, and name. Blocking differences affect the suite while advisory differences remain sourced reports.',
          },
          {
            signal: 'GROUND · PNG SHA-256',
            title: 'Optional visual grounding',
            body: 'An explicit request or deterministic miss can invoke a deployment provider for point or box candidates. Screenshot, observation, or budget drift fails closed.',
          },
          {
            signal: 'AUDIT · FORENSIC · ADVISORY',
            title: 'Optional design review',
            body: 'A screenshot plus forensic Page Context binds hierarchy, layout, typography, color, and responsive advice. The report contains no verdict, action, or repair authority.',
          },
        ],
      },
      {
        id: 'evidence',
        code: 'EVIDENCE & ACL',
        title: 'Evidence, regression, and scheduling',
        summary:
          'Retain observations, actions, assertions, and artifacts as auditable records, then run stable paths locally, in CI, or across workers.',
        href: '/guide/workflows.html',
        linkLabel: 'Review agent-session and ACL regression workflows',
        items: [
          {
            signal: 'PNG · AX · CONSOLE · ERRORS',
            title: 'Small default evidence set',
            body: 'Capture screenshots, accessibility, console, and page errors as needed. Enable HAR, trace, or video only around a diagnostic window that needs them.',
          },
          {
            signal: 'HAR · TRACE · VIDEO · DOWNLOAD',
            title: 'On-demand diagnostic artifacts',
            body: 'HAR, Chrome trace, WebM video, and bounded downloads start and stop around an explicit window instead of turning the whole session into an oversized evidence bundle.',
          },
          {
            signal: 'SESSION · EVENTS · REPORT',
            title: 'Persistent agent sessions',
            body: 'A session keeps goals, observations, typed actions, append-only events, evidence, and a terminal report while the coding agent chooses one action from each fresh page.',
          },
          {
            signal: 'CHECK · RUN · CI',
            title: 'Deterministic ACL suites',
            body: 'Encode the smallest proven path as ACL, statically admit it, then repeat actions, waits, assertions, and bounded cleanup locally and in CI.',
          },
          {
            signal: 'INVENTORY · SHARD · REMOTE',
            title: 'Capability inventory and distributed shards',
            body: 'A worker reports platform, driver, protocol, and capacity before receiving admitted shards. Remote requests cannot choose executables, credentials, or host network policy.',
          },
          {
            signal: 'PRODUCT · SPEC · INFRA',
            title: 'Distinguishable failure results',
            body: 'Reports separate product assertion failures, test-specification errors, driver or environment failures, and cleanup errors so the next owner is clear.',
          },
        ],
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
