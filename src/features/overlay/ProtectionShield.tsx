import { useEffect, useState } from "react";
import { commands, type ProtectionState, type ProtectionStatus } from "@/ipc";

const LOOK: Record<ProtectionState, { label: string; className: string }> = {
  protected: { label: "Hidden from screen share", className: "bg-success" },
  not_guaranteed: { label: "Screen-share hiding not guaranteed", className: "bg-warning" },
  unavailable: { label: "Visible in screen share", className: "bg-danger" },
  disabled: { label: "Screen-share hiding off", className: "bg-fg-muted" },
};

/** Shows whether the overlay is hidden from screen sharing (PRD UI-4a). */
export function ProtectionShield() {
  const [status, setStatus] = useState<ProtectionStatus | null>(null);

  useEffect(() => {
    void commands.getCaptureProtection().then(setStatus);
  }, []);

  if (!status) return null;
  const look = LOOK[status.state];
  return (
    <span
      role="status"
      aria-label={`${look.label}. ${status.reason}`}
      title={status.reason}
      className="flex items-center gap-1 text-xs text-fg-2"
    >
      <span
        aria-hidden="true"
        className={`inline-block h-2.5 w-2.5 rounded-full ${look.className}`}
      />
      <span className="sr-only sm:not-sr-only">{look.label}</span>
    </span>
  );
}
