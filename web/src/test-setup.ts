// Vitest setup shared by all projects. Tells React it's running under a test
// runner so `act(...)` batches updates without warning (jsdom component tests);
// harmless in the node-env unit tests.
(
  globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;
