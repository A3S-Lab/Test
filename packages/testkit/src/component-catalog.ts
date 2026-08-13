export type ComponentCatalogItem = {
  name: string;
  searchTerms?: readonly string[];
};

export type ComponentCatalogGroup = {
  name: string;
  components: readonly ComponentCatalogItem[];
};

export const COMPONENT_CATALOG: readonly ComponentCatalogGroup[] = [
  {
    name: "Structure and content",
    components: [
      { name: "Hero" },
      { name: "Section" },
      { name: "Card" },
      { name: "Article" },
      { name: "Feature Grid" },
      { name: "Media Gallery" },
      { name: "Statistic Panel", searchTerms: ["metric", "number"] },
      { name: "Quote" },
      { name: "Timeline" },
    ],
  },
  {
    name: "Navigation",
    components: [
      { name: "App Header", searchTerms: ["masthead"] },
      { name: "Navigation Bar", searchTerms: ["navbar"] },
      { name: "Sidebar Navigation", searchTerms: ["side menu"] },
      { name: "Breadcrumbs", searchTerms: ["path"] },
      { name: "Tabs" },
      { name: "Pagination", searchTerms: ["pages"] },
      { name: "Command Menu", searchTerms: ["command palette"] },
      { name: "Table of Contents", searchTerms: ["outline"] },
      { name: "Footer" },
    ],
  },
  {
    name: "Forms and input",
    components: [
      { name: "Form" },
      { name: "Search Field", searchTerms: ["query"] },
      { name: "Text Field", searchTerms: ["input"] },
      { name: "Text Area", searchTerms: ["multiline"] },
      { name: "Select Menu", searchTerms: ["dropdown"] },
      { name: "Checkbox Group" },
      { name: "Radio Group" },
      { name: "Date Picker", searchTerms: ["date input"] },
      { name: "File Upload", searchTerms: ["attachment"] },
      { name: "Verification Code Input", searchTerms: ["one-time code", "otp", "pin"] },
    ],
  },
  {
    name: "Actions and feedback",
    components: [
      { name: "Button Group" },
      { name: "Floating Action Button", searchTerms: ["fab"] },
      { name: "Alert" },
      { name: "Toast", searchTerms: ["notification popup"] },
      { name: "Progress Indicator", searchTerms: ["loading progress"] },
      { name: "Skeleton Loader", searchTerms: ["placeholder loading"] },
      { name: "Empty State" },
      { name: "Error State" },
      { name: "Confirmation Dialog", searchTerms: ["confirm modal"] },
      { name: "Tooltip" },
    ],
  },
  {
    name: "Commerce",
    components: [
      { name: "Product Card" },
      { name: "Product Gallery" },
      { name: "Pricing Table" },
      { name: "Cart Drawer", searchTerms: ["basket"] },
      { name: "Checkout Form", searchTerms: ["payment checkout"] },
      { name: "Order Summary" },
      { name: "Coupon Field", searchTerms: ["promo code"] },
      { name: "Subscription Plan" },
      { name: "Comparison Table" },
    ],
  },
  {
    name: "Data display",
    components: [
      { name: "Data Table", searchTerms: ["grid"] },
      { name: "List" },
      { name: "Description List", searchTerms: ["key value"] },
      { name: "Badge", searchTerms: ["tag chip"] },
      { name: "Avatar Group" },
      { name: "Chart", searchTerms: ["graph visualization"] },
      { name: "KPI Panel", searchTerms: ["metric indicator"] },
      { name: "Calendar" },
      { name: "Activity Feed", searchTerms: ["event log"] },
    ],
  },
  {
    name: "Identity and access",
    components: [
      { name: "Sign In Form", searchTerms: ["login"] },
      { name: "Sign Up Form", searchTerms: ["registration"] },
      { name: "Password Reset Form", searchTerms: ["forgot password"] },
      { name: "Account Menu" },
      { name: "Profile Header" },
      { name: "Permission Matrix", searchTerms: ["roles access"] },
      { name: "Session List", searchTerms: ["devices"] },
      { name: "Two-Factor Setup", searchTerms: ["2fa mfa authenticator"] },
    ],
  },
  {
    name: "Communication",
    components: [
      { name: "Comment Thread", searchTerms: ["discussion"] },
      { name: "Chat Panel" },
      { name: "Message Composer" },
      { name: "Notification Center", searchTerms: ["inbox alerts"] },
      { name: "Inbox List", searchTerms: ["messages"] },
      { name: "Contact Form" },
      { name: "Support Widget", searchTerms: ["help"] },
      { name: "Announcement Banner" },
    ],
  },
  {
    name: "Media and embeds",
    components: [
      { name: "Video Player" },
      { name: "Audio Player" },
      { name: "Image Carousel", searchTerms: ["slider"] },
      { name: "Lightbox", searchTerms: ["image viewer"] },
      { name: "Map" },
      { name: "Document Viewer", searchTerms: ["pdf preview"] },
      { name: "Code Block", searchTerms: ["source snippet"] },
      { name: "Diagram" },
    ],
  },
  {
    name: "Workspace and administration",
    components: [
      { name: "Dashboard" },
      { name: "Settings Panel" },
      { name: "Filter Bar" },
      { name: "Sort Control" },
      { name: "Bulk Action Bar" },
      { name: "Stepper", searchTerms: ["wizard steps"] },
      { name: "Kanban Board", searchTerms: ["task board"] },
      { name: "Tree View", searchTerms: ["hierarchy"] },
      { name: "Split Pane", searchTerms: ["resizable panels"] },
      { name: "Onboarding Checklist", searchTerms: ["getting started"] },
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
    if (normalize(group.name).includes(term)) return [copyGroup(group)];
    const components = group.components.filter((component) => (
      [component.name, ...(component.searchTerms ?? [])]
        .some((value) => normalize(value).includes(term))
    ));
    return components.length > 0 ? [{ name: group.name, components }] : [];
  });
}

function normalize(value: string): string {
  return value.trim().replace(/\s+/g, " ").toLocaleLowerCase("en-US");
}

function copyGroup(group: ComponentCatalogGroup): ComponentCatalogGroup {
  return { name: group.name, components: [...group.components] };
}
