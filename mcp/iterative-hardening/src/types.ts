export type ScenarioStatus = "UNTESTED" | "FAIL" | "PASS" | "BLOCKED";

export interface ScenarioRow {
  id: string;
  /** 用户可执行的测试步骤 */
  steps: string;
  /** 预期可观察结果 */
  expected: string;
  status: ScenarioStatus;
  /** 涉及源码路径（模块/文件） */
  involvedCode?: string[];
  /** 首次登记时间 ISO */
  createdAt?: string;
  /** 最近状态更新时间 ISO */
  updatedAt?: string;
  notes?: string;
}

export type AuditAngle =
  | "correctness"
  | "failure-modes"
  | "concurrency"
  | "boundaries"
  | "regression"
  | "resources";

export const AUDIT_ANGLES: AuditAngle[] = [
  "correctness",
  "failure-modes",
  "concurrency",
  "boundaries",
  "regression",
  "resources",
];

export interface VerificationEntry {
  command: string;
  exitCode: number | null;
  summary: string;
}

export interface PassReport {
  passNumber: number;
  scenarioSnapshot: Record<string, ScenarioStatus>;
  reproEvidence: string;
  auditAngle: AuditAngle;
  findings: string;
  fixes: string;
  verification: VerificationEntry[];
  retest: string;
  next: "continue" | "done";
  submittedAt: string;
}

export type SessionStatus = "idle" | "active" | "done" | "capped";

export interface HardeningSession {
  sessionId: string;
  createdAt: string;
  updatedAt: string;
  workspaceRoot: string;
  userReport: string;
  status: SessionStatus;
  passNumber: number;
  maxPasses: number;
  auditAngleIndex: number;
  scenarioMatrix: ScenarioRow[];
  passReports: PassReport[];
}

export interface NextPassGuide {
  passNumber: number;
  auditAngle: AuditAngle;
  auditAngleDescription: string;
  failingScenarios: ScenarioRow[];
  untestedScenarios: ScenarioRow[];
  steps: string[];
  passReportTemplate: string;
}

export interface DoneChecklist {
  canFinish: boolean;
  status: SessionStatus;
  reasons: string[];
  checklist: Array<{ item: string; met: boolean; detail: string }>;
}
