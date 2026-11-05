/** Test-only helpers for the browser mocks. Only `*.test.ts(x)` files may import this (eslint). */
export {
  DEFAULT_SETTINGS,
  mockAnswerBus,
  mockCalls,
  mockSessionBus,
  mockTranscriptLine,
  mockCommands,
  mockFiles,
  mockOptions,
  mockProtection,
  mockWindowCalls,
  resetMocks,
  savedAnswers,
} from "./mock";
