import type { PromptTurn } from "./prompt-turns";

type PlanningOptions = {
  message: string; chatId?: string | null; chatHistory: PromptTurn[];
  workspacePath?: string | null; maxTokens: number; tokenUsageThreshold: number;
  customSystemPromptTemplate?: string | null; isSubTask: boolean;
  proxySenderName?: string | null; enableMemoryAutoUpdate?: boolean;
};
type PlanningResult = { text: string; inputTokens: number; outputTokens: number; cachedInputTokens: number };
const sessions = new Map<string, PlanningSession>();
let parentExecutionId = "";
export function setPlanningExecution(executionId: string): void { parentExecutionId = executionId; }

const commands = Tools.SoftwareSettings as typeof Tools.SoftwareSettings & { exec(args: string[]): Promise<string> };
async function command(action: string, input: object): Promise<object> {
  return JSON.parse(await commands.exec(["--json", "legacy-tools", action, JSON.stringify(input)]));
}
type ModelJob = { status: "RUNNING" | "SUCCESS" | "FAILED"; result?: PlanningResult; error?: string };
async function runPlanningModel(input: object): Promise<PlanningResult> {
  const started = await command("chat-plan-call", input) as {executionId: string; status: "RUNNING"};
  for (;;) {
    await new Promise<void>(resolve => setTimeout(resolve, 300));
    const job = await command("model-job", {executionId: started.executionId}) as ModelJob;
    if (job.status === "FAILED") throw new Error(job.error || "Planning failed");
    if (job.status === "SUCCESS") {
      if (!job.result) throw new Error("Planning completed without a result");
      return job.result;
    }
  }
}

export async function sendPlanningRequest(options: PlanningOptions, onChunk?: (chunk: string) => void): Promise<string> {
  const requestId = options.chatId || `legacy-plan:${Date.now()}:${Math.random().toString(36).slice(2)}`;
  const session = getSession(requestId);
  session.active = true;
  try {
    const result = await runPlanningModel({ ...options, requestId, parentExecutionId });
    session.tokens = result;
    if (onChunk && result.text) onChunk(result.text);
    return result.text;
  } finally { session.active = false; }
}
export function publishPlanningState(state: {kind: string; message?: string}): void {
  if (state.message) sendIntermediateResult({ chunk: `<log>${state.message.replace(/[<&>]/g, " ")}</log>\n` });
}
class PlanningSession {
  active = false;
  tokens: PlanningResult = { text: "", inputTokens: 0, outputTokens: 0, cachedInputTokens: 0 };
  constructor(readonly id: string) {}
  cancelConversation(): void {
    if (this.active) command("chat-plan-cancel", { requestId: this.id }).catch(error => console.error("Planning cancellation failed", error));
  }
  setInputProcessingState(state: {kind: string; message?: string}): void { publishPlanningState(state); }
  getCurrentInputTokenCount(): number { return this.tokens.inputTokens; }
  getCurrentOutputTokenCount(): number { return this.tokens.outputTokens; }
  getCurrentCachedInputTokenCount(): number { return this.tokens.cachedInputTokens; }
}
function getSession(id: string): PlanningSession {
  let session = sessions.get(id);
  if (!session) { session = new PlanningSession(id); sessions.set(id, session); }
  return session;
}
export const PlanningSessions = {
  getChatInstance(_context: unknown, chatId: string): PlanningSession { return getSession(chatId); },
  getInstance(_context: unknown): PlanningSession { const chatId = getChatId(); if (!chatId) throw new Error("Deepsearch requires an active chat"); return getSession(chatId); },
  releaseChatInstance(chatId: string): void { sessions.delete(chatId); },
};
interface PreferenceEditor { putBoolean(key: string, value: boolean): PreferenceEditor; apply(): void; }
interface PreferencesContext { getSharedPreferences(name: string, mode: number): { getBoolean(key: string, initial: boolean): boolean; edit(): PreferenceEditor }; }
export const PlanningPreferences = {
  getFeatureToggleBlocking(context: unknown, key: string, initial: boolean): boolean {
    return (context as PreferencesContext).getSharedPreferences("operit2_legacy_deepsearch", 0).getBoolean(key, initial);
  },
  setFeatureToggleBlocking(context: unknown, key: string, enabled: boolean): void {
    (context as PreferencesContext).getSharedPreferences("operit2_legacy_deepsearch", 0).edit().putBoolean(key, enabled).apply();
  },
};
