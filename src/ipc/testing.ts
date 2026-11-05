/** Test-only helpers for the browser mocks. Only `*.test.ts(x)` files may import this (eslint). */
export {
  DEFAULT_SETTINGS,
  mockAnswerBus,
  mockCalls,
  mockSessionBus,
  mockCommands,
  mockProtection,
  mockWindowCalls,
  resetMocks,
} from "./mock";
