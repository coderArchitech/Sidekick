// Public API of the orchestrator: detection, prompts, routing (tech doc 3.2).
export { findName, isQuestion, shouldAnswer } from "./detect";
export { QuestionDetector, type DetectedQuestion, type SegmentIn } from "./detector";
export { useOrchestratorStore, type QueuedQuestion } from "./store";
export { useQuestionDetection } from "./useQuestionDetection";
