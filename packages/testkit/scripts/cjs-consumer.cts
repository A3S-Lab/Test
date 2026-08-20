import testkit = require("@a3s-lab/testkit");
import reactTestkit = require("@a3s-lab/testkit/react");

const options: testkit.TestKitOptions = {
  enabled: false,
  page: { id: "cjs-consumer" },
};
const provider: reactTestkit.A3STestKitProps = {
  enabled: false,
  page: { id: "cjs-consumer" },
  children: null,
};
const handshake: testkit.TestKitHandshake = {
  protocol: testkit.TESTKIT_HANDSHAKE_PROTOCOL,
  packageName: testkit.TESTKIT_PACKAGE_NAME,
  sdkVersion: "0.6.0",
  pageContextProtocol: "a3s.test.page-context/1",
  capabilities: ["bounded_snapshot"],
};

void [
  testkit.getPageContextBridge,
  testkit.installTestKit,
  testkit.registerBoundary,
  testkit.TESTKIT_HANDSHAKE_PROTOCOL,
  testkit.TESTKIT_PACKAGE_NAME,
  reactTestkit.A3STestKit,
  reactTestkit.A3STestBoundary,
  reactTestkit.A3SReviewOverlay,
  options,
  provider,
  handshake,
];
