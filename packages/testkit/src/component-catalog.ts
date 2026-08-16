export type ComponentCatalogItem = {
  name: string;
  zhCNName: string;
  searchTerms?: readonly string[];
};

export type ComponentCatalogGroup = {
  name: string;
  zhCNName: string;
  components: readonly ComponentCatalogItem[];
};

export type ComponentCatalogLocale = "en" | "zh-CN";

export const COMPONENT_CATALOG: readonly ComponentCatalogGroup[] = [
  {
    name: "Structure and content",
    zhCNName: "结构与内容",
    components: [
      { name: "Hero", zhCNName: "首屏主视觉" },
      { name: "Section", zhCNName: "区块" },
      { name: "Card", zhCNName: "卡片" },
      { name: "Article", zhCNName: "文章" },
      { name: "Feature Grid", zhCNName: "功能网格" },
      { name: "Media Gallery", zhCNName: "媒体画廊" },
      { name: "Statistic Panel", zhCNName: "统计面板", searchTerms: ["metric", "number"] },
      { name: "Quote", zhCNName: "引用" },
      { name: "Timeline", zhCNName: "时间线" },
    ],
  },
  {
    name: "Navigation",
    zhCNName: "导航",
    components: [
      { name: "App Header", zhCNName: "应用页眉", searchTerms: ["masthead"] },
      { name: "Navigation Bar", zhCNName: "导航栏", searchTerms: ["navbar"] },
      { name: "Sidebar Navigation", zhCNName: "侧边导航", searchTerms: ["side menu"] },
      { name: "Breadcrumbs", zhCNName: "面包屑", searchTerms: ["path"] },
      { name: "Tabs", zhCNName: "标签页" },
      { name: "Pagination", zhCNName: "分页", searchTerms: ["pages"] },
      { name: "Command Menu", zhCNName: "命令菜单", searchTerms: ["command palette"] },
      { name: "Table of Contents", zhCNName: "目录", searchTerms: ["outline"] },
      { name: "Footer", zhCNName: "页脚" },
    ],
  },
  {
    name: "Forms and input",
    zhCNName: "表单与输入",
    components: [
      { name: "Form", zhCNName: "表单" },
      { name: "Search Field", zhCNName: "搜索框", searchTerms: ["query"] },
      { name: "Text Field", zhCNName: "文本输入框", searchTerms: ["input"] },
      { name: "Text Area", zhCNName: "多行文本框", searchTerms: ["multiline"] },
      { name: "Select Menu", zhCNName: "选择菜单", searchTerms: ["dropdown"] },
      { name: "Checkbox Group", zhCNName: "复选框组" },
      { name: "Radio Group", zhCNName: "单选框组" },
      { name: "Date Picker", zhCNName: "日期选择器", searchTerms: ["date input"] },
      { name: "File Upload", zhCNName: "文件上传", searchTerms: ["attachment"] },
      { name: "Verification Code Input", zhCNName: "验证码输入框", searchTerms: ["one-time code", "otp", "pin"] },
    ],
  },
  {
    name: "Actions and feedback",
    zhCNName: "操作与反馈",
    components: [
      { name: "Button Group", zhCNName: "按钮组" },
      { name: "Floating Action Button", zhCNName: "悬浮操作按钮", searchTerms: ["fab"] },
      { name: "Alert", zhCNName: "提示" },
      { name: "Toast", zhCNName: "轻提示", searchTerms: ["notification popup"] },
      { name: "Progress Indicator", zhCNName: "进度指示器", searchTerms: ["loading progress"] },
      { name: "Skeleton Loader", zhCNName: "骨架屏", searchTerms: ["placeholder loading"] },
      { name: "Empty State", zhCNName: "空状态" },
      { name: "Error State", zhCNName: "错误状态" },
      { name: "Confirmation Dialog", zhCNName: "确认对话框", searchTerms: ["confirm modal"] },
      { name: "Tooltip", zhCNName: "工具提示" },
    ],
  },
  {
    name: "Commerce",
    zhCNName: "电商",
    components: [
      { name: "Product Card", zhCNName: "商品卡片" },
      { name: "Product Gallery", zhCNName: "商品画廊" },
      { name: "Pricing Table", zhCNName: "定价表" },
      { name: "Cart Drawer", zhCNName: "购物车抽屉", searchTerms: ["basket"] },
      { name: "Checkout Form", zhCNName: "结账表单", searchTerms: ["payment checkout"] },
      { name: "Order Summary", zhCNName: "订单摘要" },
      { name: "Coupon Field", zhCNName: "优惠码输入框", searchTerms: ["promo code"] },
      { name: "Subscription Plan", zhCNName: "订阅方案" },
      { name: "Comparison Table", zhCNName: "对比表" },
    ],
  },
  {
    name: "Data display",
    zhCNName: "数据展示",
    components: [
      { name: "Data Table", zhCNName: "数据表格", searchTerms: ["grid"] },
      { name: "List", zhCNName: "列表" },
      { name: "Description List", zhCNName: "描述列表", searchTerms: ["key value"] },
      { name: "Badge", zhCNName: "徽标", searchTerms: ["tag chip"] },
      { name: "Avatar Group", zhCNName: "头像组" },
      { name: "Chart", zhCNName: "图表", searchTerms: ["graph visualization"] },
      { name: "KPI Panel", zhCNName: "指标面板", searchTerms: ["metric indicator"] },
      { name: "Calendar", zhCNName: "日历" },
      { name: "Activity Feed", zhCNName: "动态列表", searchTerms: ["event log"] },
    ],
  },
  {
    name: "Identity and access",
    zhCNName: "身份与访问",
    components: [
      { name: "Sign In Form", zhCNName: "登录表单", searchTerms: ["login"] },
      { name: "Sign Up Form", zhCNName: "注册表单", searchTerms: ["registration"] },
      { name: "Password Reset Form", zhCNName: "重置密码表单", searchTerms: ["forgot password"] },
      { name: "Account Menu", zhCNName: "账户菜单" },
      { name: "Profile Header", zhCNName: "个人资料页眉" },
      { name: "Permission Matrix", zhCNName: "权限矩阵", searchTerms: ["roles access"] },
      { name: "Session List", zhCNName: "会话列表", searchTerms: ["devices"] },
      { name: "Two-Factor Setup", zhCNName: "双重验证设置", searchTerms: ["2fa mfa authenticator"] },
    ],
  },
  {
    name: "Communication",
    zhCNName: "沟通",
    components: [
      { name: "Comment Thread", zhCNName: "评论串", searchTerms: ["discussion"] },
      { name: "Chat Panel", zhCNName: "聊天面板" },
      { name: "Message Composer", zhCNName: "消息编辑器" },
      { name: "Notification Center", zhCNName: "通知中心", searchTerms: ["inbox alerts"] },
      { name: "Inbox List", zhCNName: "收件箱列表", searchTerms: ["messages"] },
      { name: "Contact Form", zhCNName: "联系表单" },
      { name: "Support Widget", zhCNName: "支持工具", searchTerms: ["help"] },
      { name: "Announcement Banner", zhCNName: "公告横幅" },
    ],
  },
  {
    name: "Media and embeds",
    zhCNName: "媒体与嵌入内容",
    components: [
      { name: "Video Player", zhCNName: "视频播放器" },
      { name: "Audio Player", zhCNName: "音频播放器" },
      { name: "Image Carousel", zhCNName: "图片轮播", searchTerms: ["slider"] },
      { name: "Lightbox", zhCNName: "灯箱", searchTerms: ["image viewer"] },
      { name: "Map", zhCNName: "地图" },
      { name: "Document Viewer", zhCNName: "文档查看器", searchTerms: ["pdf preview"] },
      { name: "Code Block", zhCNName: "代码块", searchTerms: ["source snippet"] },
      { name: "Diagram", zhCNName: "示意图" },
    ],
  },
  {
    name: "Workspace and administration",
    zhCNName: "工作区与管理",
    components: [
      { name: "Dashboard", zhCNName: "仪表盘" },
      { name: "Settings Panel", zhCNName: "设置面板" },
      { name: "Filter Bar", zhCNName: "筛选栏" },
      { name: "Sort Control", zhCNName: "排序控件" },
      { name: "Bulk Action Bar", zhCNName: "批量操作栏" },
      { name: "Stepper", zhCNName: "步骤器", searchTerms: ["wizard steps"] },
      { name: "Kanban Board", zhCNName: "看板", searchTerms: ["task board"] },
      { name: "Tree View", zhCNName: "树形视图", searchTerms: ["hierarchy"] },
      { name: "Split Pane", zhCNName: "分栏面板", searchTerms: ["resizable panels"] },
      { name: "Onboarding Checklist", zhCNName: "上手检查清单", searchTerms: ["getting started"] },
    ],
  },
] as const;

export function componentCatalogSize(): number {
  return COMPONENT_CATALOG.reduce(
    (count, group) => count + group.components.length,
    0,
  );
}

export function filterComponentCatalog(query: string): ComponentCatalogGroup[] {
  const term = normalize(query).slice(0, 128);
  if (!term) return COMPONENT_CATALOG.map(copyGroup);
  return COMPONENT_CATALOG.flatMap((group) => {
    if ([group.name, group.zhCNName].some((value) => normalize(value).includes(term))) {
      return [copyGroup(group)];
    }
    const components = group.components.filter((component) => (
      [component.name, component.zhCNName, ...(component.searchTerms ?? [])]
        .some((value) => normalize(value).includes(term))
    ));
    return components.length > 0 ? [{ name: group.name, zhCNName: group.zhCNName, components }] : [];
  });
}

export function componentCatalogItemLabel(
  component: ComponentCatalogItem,
  locale: ComponentCatalogLocale,
): string {
  return locale === "zh-CN" ? component.zhCNName : component.name;
}

export function localizeComponentCatalogName(
  value: string,
  locale: ComponentCatalogLocale,
): string {
  const normalized = normalize(value);
  const component = COMPONENT_CATALOG
    .flatMap((group) => group.components)
    .find((candidate) => (
      normalize(candidate.name) === normalized
      || normalize(candidate.zhCNName) === normalized
    ));
  return component ? componentCatalogItemLabel(component, locale) : value;
}

function normalize(value: string): string {
  return value.trim().replace(/\s+/g, " ").toLocaleLowerCase("en-US");
}

function copyGroup(group: ComponentCatalogGroup): ComponentCatalogGroup {
  return {
    name: group.name,
    zhCNName: group.zhCNName,
    components: [...group.components],
  };
}
