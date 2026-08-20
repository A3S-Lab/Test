import {
  getPageContextBridge,
  installTestKit,
  registerBoundary,
  TESTKIT_HANDSHAKE_PROTOCOL,
  TESTKIT_PACKAGE_NAME,
  type TestKitHandshake,
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
const handshake: TestKitHandshake = {
  protocol: TESTKIT_HANDSHAKE_PROTOCOL,
  packageName: TESTKIT_PACKAGE_NAME,
  sdkVersion: "0.6.2",
  pageContextProtocol: "a3s.test.page-context/1",
  capabilities: ["bounded_snapshot"],
};

void [
  getPageContextBridge,
  installTestKit,
  registerBoundary,
  TESTKIT_HANDSHAKE_PROTOCOL,
  TESTKIT_PACKAGE_NAME,
  A3STestKit,
  A3STestBoundary,
  A3SReviewOverlay,
  options,
  provider,
  handshake,
];
