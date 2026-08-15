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

void [
  testkit.getPageContextBridge,
  testkit.installTestKit,
  testkit.registerBoundary,
  reactTestkit.A3STestKit,
  reactTestkit.A3STestBoundary,
  reactTestkit.A3SReviewOverlay,
  options,
  provider,
];
