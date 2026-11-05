// Public API of the orchestrator: detection, prompts, routing (tech doc 3.2).
export { findName, isQuestion, shouldAnswer } from "./detect";
export { QuestionDetector, type DetectedQuestion, type SegmentIn } from "./detector";
export { useOrchestratorStore, type QueuedQuestion } from "./store";
export { useQuestionDetection } from "./useQuestionDetection";
export { AnswerRouter, type AnswerView, type AnswerStatus } from "./answerer";
export { useAnswersStore } from "./answersStore";
export { modelPlan, type ModelPlan } from "./models";
export { NOT_FOUND, buildPrompt, citedSources, type SourceRef } from "./prompt";
export { activeRouter, askChat, useAnswering } from "./useAnswering";
export { CHAT_HISTORY_TURNS, chatHistory, useChatStore } from "./chatStore";
