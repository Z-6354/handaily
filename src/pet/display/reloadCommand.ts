export type ReloadSource =
  | "rust"
  | "menu"
  | "boot"
  | "visibility"
  | "manual";

export type ReloadCommandKind =
  | "nudge"
  | "pet-reload"
  | "switch-model"
  | "boot-fallback";

export interface ReloadCommand {
  kind: ReloadCommandKind;
  source: ReloadSource;
  /** Optional trace id for logs / hardening checklist */
  trace?: string;
}

export function reloadCommand(
  kind: ReloadCommandKind,
  source: ReloadSource,
  trace?: string,
): ReloadCommand {
  return { kind, source, trace };
}

/** Accept legacy string reasons from call sites during migration */
export function normalizeReloadCommand(input: ReloadCommand | string): ReloadCommand {
  if (typeof input !== "string") return input;
  if (input === "boot-fallback") return reloadCommand("boot-fallback", "boot", input);
  if (input.includes("fallback")) {
    return reloadCommand("pet-reload", "manual", input);
  }
  return reloadCommand("pet-reload", input === "nudge" ? "rust" : "manual", input);
}

export function reloadCommandLabel(cmd: ReloadCommand): string {
  return cmd.trace ?? `${cmd.kind}@${cmd.source}`;
}
