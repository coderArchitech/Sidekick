import type { Tool, Trigger } from "@/ipc";

export const TOOL_LABELS: Record<Tool, { label: string; detail: string }> = {
  doc_search: { label: "Document search", detail: "Reads your attached documents." },
  web_search: { label: "Web search", detail: "Sends search queries to your web search provider." },
  calculator: { label: "Calculator", detail: "Does arithmetic locally." },
};

export const TRIGGER_LABELS: Record<Trigger, string> = {
  question_detected: "Detected questions",
  chat: "Chat",
  screenshot: "Screenshot questions",
};
