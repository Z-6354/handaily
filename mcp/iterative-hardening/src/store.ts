import fs from "node:fs";
import path from "node:path";
import { randomUUID } from "node:crypto";
import {
  AUDIT_ANGLES,
  type AuditAngle,
  type DoneChecklist,
  type HardeningSession,
  type NextPassGuide,
  type PassReport,
  type ScenarioRow,
  type ScenarioStatus,
  type SessionStatus,
  type VerificationEntry,
} from "./types.js";

const SESSION_DIR = ".iterative-hardening";
const SESSION_FILE = "session.json";

const ANGLE_DESCRIPTIONS: Record<AuditAngle, string> = {
  correctness: "Wrong conditions, off-by-one, null/empty",
  "failure-modes": "Swallowed errors, partial cleanup, dead ends",
  concurrency: "Races, stale flags, re-entrancy, timer reset",
  boundaries: "IPC contracts, types, multi-window lifecycle",
  regression: "Tests; run existing suite; add test for fixed bug",
  resources: "Leaks, intervals not cleared, windows not destroyed",
};

const PASS_REPORT_TEMPLATE = `## Loop pass N
**Scenario Matrix:** S1 PASS, S2 FAIL, S3 UNTESTED …
**Repro evidence:** [what actually happened on failing rows]
**Audit angle:** [correctness | failure-modes | concurrency | boundaries | regression | resources]
**Findings:** [none | list with severity]
**Fixes:** [files touched, one line each]
**Verification commands:** [command → exit code / summary]
**Retest:** [re-ran S1–Sn; which still fail]
**Next:** continue loop | done`;

export class HardeningStore {
  private workspaceRoot: string;

  constructor(workspaceRoot?: string) {
    this.workspaceRoot = workspaceRoot ?? process.env.HANDAILY_ROOT ?? process.cwd();
  }

  private sessionPath(): string {
    return path.join(this.workspaceRoot, SESSION_DIR, SESSION_FILE);
  }

  private ensureDir(): void {
    const dir = path.join(this.workspaceRoot, SESSION_DIR);
    fs.mkdirSync(dir, { recursive: true });
  }

  load(): HardeningSession | null {
    const p = this.sessionPath();
    if (!fs.existsSync(p)) return null;
    try {
      return JSON.parse(fs.readFileSync(p, "utf8")) as HardeningSession;
    } catch {
      return null;
    }
  }

  save(session: HardeningSession): void {
    this.ensureDir();
    session.updatedAt = new Date().toISOString();
    fs.writeFileSync(this.sessionPath(), JSON.stringify(session, null, 2), "utf8");
  }

  init(userReport: string, scenarios: ScenarioRow[]): HardeningSession {
    const now = new Date().toISOString();
    const session: HardeningSession = {
      sessionId: randomUUID(),
      createdAt: now,
      updatedAt: now,
      workspaceRoot: this.workspaceRoot,
      userReport,
      status: "active",
      passNumber: 0,
      maxPasses: 12,
      auditAngleIndex: 0,
      scenarioMatrix: scenarios.map((s) => ({
        ...s,
        status: s.status ?? "UNTESTED",
      })),
      passReports: [],
    };
    this.save(session);
    return session;
  }

  requireSession(): HardeningSession {
    const s = this.load();
    if (!s || s.status === "idle") {
      throw new Error("No active session. Call hardening_init first.");
    }
    return s;
  }

  getStatus(): { session: HardeningSession | null; message: string } {
    const session = this.load();
    if (!session) {
      return {
        session: null,
        message: "No session. Use hardening_init to start Phase 0 (Scenario Matrix).",
      };
    }
    const failing = session.scenarioMatrix.filter((r) => r.status === "FAIL");
    const untested = session.scenarioMatrix.filter((r) => r.status === "UNTESTED");
    const blocked = session.scenarioMatrix.filter((r) => r.status === "BLOCKED");
    return {
      session,
      message: [
        `Status: ${session.status}`,
        `Pass: ${session.passNumber}/${session.maxPasses}`,
        `Scenarios: ${session.scenarioMatrix.length} total`,
        `FAIL=${failing.length} UNTESTED=${untested.length} BLOCKED=${blocked.length}`,
        failing.length ? `Failing: ${failing.map((r) => r.id).join(", ")}` : "",
      ]
        .filter(Boolean)
        .join(" | "),
    };
  }

  nextPass(): NextPassGuide {
    const session = this.requireSession();
    if (session.status === "done") {
      throw new Error("Session already done. Use hardening_init for a new loop.");
    }
    if (session.status === "capped") {
      throw new Error(`Loop capped at ${session.maxPasses} passes. Report blockers to user.`);
    }
    if (session.passNumber >= session.maxPasses) {
      session.status = "capped";
      this.save(session);
      throw new Error(`Loop capped at ${session.maxPasses} passes.`);
    }

    const passNumber = session.passNumber + 1;
    const auditAngle = AUDIT_ANGLES[session.auditAngleIndex % AUDIT_ANGLES.length];
    const failing = session.scenarioMatrix.filter((r) => r.status === "FAIL");
    const untested = session.scenarioMatrix.filter((r) => r.status === "UNTESTED");

    return {
      passNumber,
      auditAngle,
      auditAngleDescription: ANGLE_DESCRIPTIONS[auditAngle],
      failingScenarios: failing,
      untestedScenarios: untested,
      steps: [
        "1. REPRODUCE — run failing scenarios; capture actual vs expected",
        "2. AUDIT — use the assigned audit angle; list findings",
        "3. FIX — minimal diff; root cause fix only",
        "4. VERIFY — run build/test commands with evidence",
        "5. RETEST — re-run entire Scenario Matrix, not just S1",
        "6. Submit via hardening_submit_pass",
      ],
      passReportTemplate: PASS_REPORT_TEMPLATE,
    };
  }

  submitPass(input: {
    reproEvidence: string;
    findings: string;
    fixes: string;
    verification: VerificationEntry[];
    retest: string;
    scenarioUpdates: Record<string, ScenarioStatus>;
    next: "continue" | "done";
  }): { session: HardeningSession; passReport: PassReport; warnings: string[] } {
    const session = this.requireSession();
    const warnings: string[] = [];

    if (session.status === "done" || session.status === "capped") {
      throw new Error(`Cannot submit pass: session status is ${session.status}`);
    }

    const passNumber = session.passNumber + 1;
    if (passNumber > session.maxPasses) {
      session.status = "capped";
      this.save(session);
      throw new Error(`Loop capped at ${session.maxPasses} passes.`);
    }

    for (const [id, status] of Object.entries(input.scenarioUpdates)) {
      const row = session.scenarioMatrix.find((r) => r.id === id);
      if (!row) {
        warnings.push(`Unknown scenario id: ${id}`);
        continue;
      }
      row.status = status;
    }

    const auditAngle = AUDIT_ANGLES[session.auditAngleIndex % AUDIT_ANGLES.length];
    const snapshot: Record<string, ScenarioStatus> = {};
    for (const row of session.scenarioMatrix) {
      snapshot[row.id] = row.status;
    }

    const passReport: PassReport = {
      passNumber,
      scenarioSnapshot: snapshot,
      reproEvidence: input.reproEvidence,
      auditAngle,
      findings: input.findings,
      fixes: input.fixes,
      verification: input.verification,
      retest: input.retest,
      next: input.next,
      submittedAt: new Date().toISOString(),
    };

    session.passNumber = passNumber;
    session.auditAngleIndex = (session.auditAngleIndex + 1) % AUDIT_ANGLES.length;
    session.passReports.push(passReport);

    const doneCheck = this.checkDone(session);
    if (input.next === "done" && !doneCheck.canFinish) {
      warnings.push("Requested done but exit criteria not met. Session stays active.");
      session.status = "active";
    } else if (doneCheck.canFinish && input.next === "done") {
      session.status = "done";
    } else if (
      session.scenarioMatrix.some((r) => r.status === "FAIL") ||
      (input.findings.trim() && input.findings.trim().toLowerCase() !== "none")
    ) {
      session.status = "active";
    } else if (doneCheck.canFinish) {
      session.status = "done";
    }

    if (session.passNumber >= session.maxPasses && session.status === "active") {
      const stillFailing = session.scenarioMatrix.some((r) => r.status === "FAIL");
      if (stillFailing) {
        session.status = "capped";
        warnings.push(`Reached ${session.maxPasses} passes with failing scenarios.`);
      }
    }

    this.save(session);
    return { session, passReport, warnings };
  }

  addScenario(row: Omit<ScenarioRow, "status"> & { status?: ScenarioStatus }): HardeningSession {
    const session = this.requireSession();
    if (session.scenarioMatrix.some((r) => r.id === row.id)) {
      throw new Error(`Scenario ${row.id} already exists.`);
    }
    session.scenarioMatrix.push({
      ...row,
      status: row.status ?? "UNTESTED",
    });
    if (session.status === "done") {
      session.status = "active";
    }
    this.save(session);
    return session;
  }

  updateScenario(id: string, status: ScenarioStatus, notes?: string): HardeningSession {
    const session = this.requireSession();
    const row = session.scenarioMatrix.find((r) => r.id === id);
    if (!row) throw new Error(`Scenario ${id} not found.`);
    row.status = status;
    if (notes !== undefined) row.notes = notes;
    if (status === "FAIL" && session.status === "done") {
      session.status = "active";
    }
    this.save(session);
    return session;
  }

  checkDone(session?: HardeningSession): DoneChecklist {
    const s = session ?? this.load();
    const reasons: string[] = [];
    if (!s) {
      return {
        canFinish: false,
        status: "idle",
        reasons: ["No session"],
        checklist: [],
      };
    }

    const allPass = s.scenarioMatrix.every((r) => r.status === "PASS");
    const anyFail = s.scenarioMatrix.some((r) => r.status === "FAIL");
    const anyUntested = s.scenarioMatrix.some((r) => r.status === "UNTESTED");
    const anyBlocked = s.scenarioMatrix.some((r) => r.status === "BLOCKED");
    const latestFindings =
      s.passReports.length > 0 ? s.passReports[s.passReports.length - 1].findings.trim() : "";
    const findingsClean =
      !latestFindings || latestFindings.toLowerCase() === "none";
    const hasVerification =
      s.passReports.length > 0 &&
      s.passReports[s.passReports.length - 1].verification.length > 0;
    const auditCycleComplete = s.auditAngleIndex === 0 && s.passNumber >= AUDIT_ANGLES.length;

    const checklist = [
      {
        item: "Every scenario PASS",
        met: allPass && !anyFail && !anyUntested,
        detail: anyUntested
          ? `UNTESTED: ${s.scenarioMatrix.filter((r) => r.status === "UNTESTED").map((r) => r.id).join(", ")}`
          : anyFail
            ? `FAIL: ${s.scenarioMatrix.filter((r) => r.status === "FAIL").map((r) => r.id).join(", ")}`
            : "all PASS",
      },
      {
        item: "No BLOCKED without user notified",
        met: !anyBlocked,
        detail: anyBlocked
          ? `BLOCKED: ${s.scenarioMatrix.filter((r) => r.status === "BLOCKED").map((r) => r.id).join(", ")}`
          : "none",
      },
      {
        item: "Latest pass findings: none",
        met: findingsClean,
        detail: findingsClean ? "none" : latestFindings,
      },
      {
        item: "Verification commands run this session",
        met: hasVerification,
        detail: hasVerification ? "present in latest pass" : "missing",
      },
      {
        item: "Full audit angle cycle (6 angles)",
        met: auditCycleComplete || s.status === "done",
        detail: `passNumber=${s.passNumber}, angleIndex=${s.auditAngleIndex}`,
      },
    ];

    if (!allPass || anyFail || anyUntested) reasons.push("Scenario matrix not all PASS");
    if (anyBlocked) reasons.push("BLOCKED scenarios remain");
    if (!findingsClean) reasons.push("Latest audit findings not clean");
    if (!hasVerification && s.passNumber > 0) reasons.push("No verification evidence in latest pass");

    const canFinish =
      allPass &&
      !anyFail &&
      !anyUntested &&
      !anyBlocked &&
      findingsClean &&
      hasVerification &&
      s.passNumber > 0;

    return {
      canFinish,
      status: s.status,
      reasons,
      checklist,
    };
  }

  getProtocolMarkdown(): string {
    return `# Iterative Code Hardening (循环修复)

## Iron Law
NO "DONE" UNTIL THE SCENARIO MATRIX IS ALL PASS AND ONE FULL AUDIT CYCLE IS CLEAN.

## Loop
1. hardening_init — Phase 0 Scenario Matrix
2. hardening_next_pass — get pass N + audit angle
3. REPRODUCE → AUDIT → FIX → VERIFY → RETEST (all scenarios)
4. hardening_submit_pass — record pass report
5. hardening_can_finish — check exit criteria
6. Repeat until done or 12 pass cap

## Audit Angles (rotate one per pass)
${AUDIT_ANGLES.map((a, i) => `${i + 1}. ${a} — ${ANGLE_DESCRIPTIONS[a]}`).join("\n")}

## Red Flags
- "Fixed" after one change + build only
- Retest only S1, not full matrix
- Claim done with UNTESTED/BLOCKED rows
- "Should work now" without retest evidence
`;
  }
}
