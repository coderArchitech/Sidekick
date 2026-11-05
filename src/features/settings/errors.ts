import type { SidekickError } from "@/ipc";

/** User-facing text for key and provider errors, chosen by code (tech doc 5.4). */
export function keyErrorText(error: SidekickError, providerName: string): string {
  switch (error.code) {
    case "KEY_INVALID":
      return `${providerName} rejected this key. Check it and try again.`;
    case "KEY_MISSING":
      return "Paste a key first.";
    case "RATE_LIMITED":
    case "PROVIDER_UNAVAILABLE":
      return `Couldn't reach ${providerName}. Check your connection and try again.`;
    case "PERMISSION_DENIED":
      return "Sidekick couldn't access the OS keychain. Unlock it or allow access, then try again.";
    default:
      return error.message;
  }
}
