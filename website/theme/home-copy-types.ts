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

export type BenchmarkCopy = {
  title: string;
  body: string;
  tableCaption: string;
  dimension: string;
  candidate: string;
  baseline: string;
  metrics: {
    success: string;
    staleReference: string;
    evidence: string;
    latency: string;
  };
  labels: {
    mainRuns: string;
    staleRejected: string;
    pageMutations: string;
    artifactRuns: string;
    versusDirect: string;
    hostBaseline: string;
    lockedProtocol: string;
    tasks: string;
    repetitions: string;
  };
  limitation: string;
  reportLink: string;
};

export type LocalizedCopy = {
  heroTitle: [string, string];
  heroBody: string;
  startExperience: string;
  readDocs: string;
  proofItems: [
    { title: string; body: string },
    { title: string; body: string },
    { title: string; body: string },
  ];
  installTitle: string;
  installBody: string;
  testkitInstallLink: string;
  installTabs: string;
  installPackage: string;
  installNote: string;
  installCandidateNote: string;
  copy: string;
  copied: string;
  copyError: string;
  benchmark: BenchmarkCopy;
  packetTitle: string;
  packetBody: string;
  packetLabel: string;
  packetLines: string[];
  packetTrust: string;
  packetLink: string;
  quickStartTitle: string;
  quickStartBody: string;
  quickStartSteps: [
    { title: string; body: string; command: string },
    { title: string; body: string; command: string },
    { title: string; body: string; command: string },
  ];
  quickStartLink: string;
  faqTitle: string;
  faqBody: string;
  faqItems: Array<{ question: string; answer: string }>;
  ctaTitle: string;
  ctaBody: string;
  quickStart: string;
  testkitGuide: string;
  footer: string;
  experience: ExperienceCopy;
};
