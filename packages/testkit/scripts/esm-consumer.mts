import {
  getPageContextBridge,
  installTestKit,
  registerBoundary,
  type TestKitOptions,
} from "@a3s-lab/testkit";
import {
  A3SReviewOverlay,
  A3STestBoundary,
  A3STestKit,
  type A3STestKitProps,
} from "@a3s-lab/testkit/react";

const options: TestKitOptions = {
  enabled: false,
  page: { id: "esm-consumer" },
};
const provider: A3STestKitProps = {
  enabled: false,
  page: { id: "esm-consumer" },
  children: null,
};

void [
  getPageContextBridge,
  installTestKit,
  registerBoundary,
  A3STestKit,
  A3STestBoundary,
  A3SReviewOverlay,
  options,
  provider,
];
