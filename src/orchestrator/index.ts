// Public API of the orchestrator: detection, prompts, routing (tech doc 3.2).
export { findName, isQuestion, shouldAnswer } from "./detect";
export { QuestionDetector, type DetectedQuestion, type SegmentIn } from "./detector";
export { useOrchestratorStore, type QueuedQuestion } from "./store";
export { useQuestionDetection } from "./useQuestionDetection";
export { AnswerRouter, type AnswerView, type AnswerStatus } from "./answerer";
export { useAnswersStore } from "./answersStore";
export { modelPlan, sessionPlan, type ModelPlan } from "./models";
export { NOT_FOUND, buildPrompt, citedSources, type SourceRef } from "./prompt";
export {
  activeRouter,
  askChat,
  askFollowUp,
  askScreenshotAction,
  captureAndAnswer,
  useAnswering,
} from "./useAnswering";
export {
  SCREENSHOT_ACTIONS,
  screenshotAction,
  type ScreenshotActionKind,
} from "./screenshotActions";
export { FOLLOW_UPS, canFollowUp, followUpInput, type FollowUpKind } from "./followUps";
export { CHAT_HISTORY_TURNS, chatHistory, useChatStore } from "./chatStore";
export { NOTES_INTERVAL_MS, NotesJob, parseSummary, type NotesDeps } from "./notes";
export { useNotesStore, type NotesActivity } from "./notesStore";
export { useNotes } from "./useNotes";
export { agendaUpdates, keywords, matchAgenda, mentions, parseAgendaReply } from "./agenda";
export { BRIEF_SYSTEM, NO_MATERIAL, briefPrompt, writeBrief, type BriefDeps } from "./brief";
export { RECAP_MINUTES, recap, recapWindow, type RecapMinutes } from "./recap";
export { nextSkillId, skillFor, skillsFor, useSkillsStore } from "./skills";
export { MAX_TOOL_CALLS, calculate, parseStep, runAgent, type ToolResult } from "./agent";
