import { invoke } from "@tauri-apps/api/core";

export interface PerformanceSnapshot {
  systemCpuPercent: number;
  systemMemoryUsedBytes: number;
  systemMemoryTotalBytes: number;
  systemMemoryPercent: number;
  appCpuPercent: number;
  appMemoryWorkingSetBytes: number;
  appMemoryPrivateBytes: number;
  processName: string;
}

export interface Segment {
  started_at: string;
  ended_at: string | null;
  duration_ms: number;
  app_name: string;
  exe_path: string;
  window_title: string;
  is_idle: boolean;
  aggregation_key: string;
  icon?: string | null;
  /** foreground | audio */
  source_type?: string;
  /** music | video | chat | other */
  audio_activity?: string;
  /** 活动内容摘要，如项目名、网页标题 */
  activity_label?: string | null;
}

export interface StatusPayload {
  tracking: boolean;
  open_segment: Segment | null;
  foreground: ForegroundPayload | null;
}

export interface ForegroundPayload {
  app_name: string;
  exe_path: string;
  window_title: string;
  is_idle: boolean;
  captured_at: string;
}

export interface OverviewPayload {
  foreground_ms: number;
  background_ms: number;
  app_usage_ms: number;
  companion_ms: number;
  switch_count: number;
  top_app: string | null;
  top_app_display: string | null;
}

export interface AutostartStatus {
  enabled: boolean;
  supported: boolean;
}

export interface AppBreakdownItem {
  key: string;
  display_name: string;
  ms: number;
  icon?: string | null;
}

export interface HeatmapDay {
  label: string;
  date: string;
  total_ms: number;
  segment_count: number;
  slots: number[];
  work_types: (string | null)[];
  summaries: (string | null)[];
}

export interface WorkType {
  id: string;
  name: string;
  color: string;
  builtin: boolean;
}

export interface WorkTypeConfig {
  types: WorkType[];
}

export interface PeriodSummary {
  id: number;
  started_at: string;
  ended_at: string;
  work_type: string;
  summary: string;
  trigger: string;
  created_at: string;
}

export interface PetRemarkLine {
  text: string;
  animation?: string | null;
}

export interface PetStatus {
  enabled: boolean;
  visible: boolean;
  power_mode: string;
  scale: number;
  remark_interval_sec: number;
  model_id: string;
  model_name: string;
  animations: string[];
  idle_animation?: string | null;
  click_animation?: string | null;
  boot_animation?: string | null;
  return_idle_animation?: string | null;
  drag_animation?: string | null;
  random_animations: string[];
  random_min_sec: number;
  random_max_sec: number;
  lines: PetRemarkLine[];
}

export interface PetAnimationMeta {
  animations: string[];
  idle_animation?: string | null;
  click_animation?: string | null;
  boot_animation?: string | null;
  return_idle_animation?: string | null;
  drag_animation?: string | null;
  random_animations: string[];
  random_min_sec: number;
  random_max_sec: number;
  lines: PetRemarkLine[];
}

export interface PetModelInfo {
  id: string;
  name: string;
  builtin: boolean;
}

export interface PetConfig {
  model_id: string;
  model_name: string;
  asset_base: string;
  config_file?: string | null;
  skel_file: string;
  atlas_file: string;
  png_file: string;
  use_file_src: boolean;
  power_mode: string;
  scale: number;
  animations: string[];
  idle_animation?: string | null;
  click_animation?: string | null;
  boot_animation?: string | null;
  return_idle_animation?: string | null;
  drag_animation?: string | null;
  random_animations: string[];
  random_min_sec: number;
  random_max_sec: number;
  lines: PetRemarkLine[];
  window_width: number;
  window_height: number;
  offset_x: number;
  offset_y: number;
}

export interface PetImportFilesPayload {
  name: string;
  skel_b64: string;
  atlas_b64: string;
  png_b64: string;
}

export interface PetStageFilesPayload {
  skel_b64: string;
  atlas_b64: string;
  png_b64: string;
  skel_name?: string;
  atlas_name?: string;
  png_name?: string;
}

export interface PetImportStagingPreview {
  source: string;
  folder_path?: string | null;
  skel_file: string;
  atlas_file: string;
  png_file: string;
  config_file?: string | null;
  config_generated: boolean;
}

export interface TimelinePage {
  total: number;
  items: Segment[];
}

export interface VaultStatus {
  initialized: boolean;
  has_password: boolean;
  unlocked: boolean;
}

export interface VaultEntry {
  id: number;
  name: string;
  website_url: string;
  created_at: string;
  updated_at: string;
}

export interface VaultEntryInput {
  name: string;
  website_url?: string;
  secret: string;
}

export interface AnalysisStats {
  text_count: number;
  screenshot_count: number;
  skipped_screenshot_count: number;
  system_cpu_percent: number;
}

export interface ActivityInsight {
  id: number;
  started_at: string;
  app_name: string;
  source: string;
  category: string;
  summary: string;
  confidence: number;
}

export interface DailyMetrics {
  date: string;
  mouse_clicks: number;
  key_strokes: number;
  keyboard_text: string;
  files_created: number;
  files_modified: number;
}

export interface VendorTestResult {
  ok: boolean;
  message: string;
  imported_text?: number;
  imported_vision?: number;
}

export interface AiVendor {
  id: string;
  name: string;
  base_url: string;
  api_style: string;
  vault_entry_id: number | null;
}

export interface AiModelEntry {
  id: string;
  name: string;
  vendor_id: string;
  kind: "text" | "vision" | "thinking";
  custom: boolean;
}

export interface AiConfig {
  text_vendor_id: string;
  text_model: string;
  vision_vendor_id: string;
  vision_model: string;
  thinking_vendor_id: string;
  thinking_model: string;
  vendors: AiVendor[];
  custom_models: AiModelEntry[];
  imported_models: AiModelEntry[];
}

/** 补齐旧版/缺字段配置，避免 save 时丢失 imported_models */
export function normalizeAiConfig(cfg: AiConfig): AiConfig {
  return {
    ...cfg,
    thinking_vendor_id: cfg.thinking_vendor_id || cfg.text_vendor_id,
    thinking_model: cfg.thinking_model || "",
    custom_models: cfg.custom_models ?? [],
    imported_models: cfg.imported_models ?? [],
  };
}

export interface CharacterProfileData {
  name: string;
  source: string;
  introduction: string;
  personality: string[];
  speech_style: string;
  sample_lines: string[];
  relationships: string;
  taboos: string[];
  extra: Record<string, string>;
}

export interface CharacterProfile {
  id: number;
  slug: string;
  name: string;
  source: string;
  raw_text: string;
  profile_json: CharacterProfileData;
  skill_md: string;
  persona_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface CharacterOpResult {
  profile: CharacterProfile;
  message: string;
}

export interface AiModelOption {
  id: string;
  name: string;
  custom: boolean;
}

export interface PersonaInfo {
  id: string;
  name: string;
  source: string;
  description: string;
  active: boolean;
  has_profile: boolean;
  is_builtin: boolean;
}

export interface PersonaDetail {
  id: string;
  name: string;
  source: string;
  description: string;
  active: boolean;
  skill_md: string;
  profile_json: CharacterProfileData;
  is_builtin: boolean;
}

export interface PersonaImportFile {
  filename: string;
  content: string;
}

export interface PersonaImportResult {
  imported_ids: string[];
  message: string;
}

export interface PersonaImportProgressEvent {
  step: string;
  message: string;
  step_index: number;
  step_total: number;
}

export interface PetLinesImportProgressEvent {
  step: string;
  message: string;
  step_index: number;
  step_total: number;
}

export interface PersonaUpdateInput {
  name: string;
  source: string;
  description: string;
  skill_md: string;
  profile_json: CharacterProfileData;
}

export interface PersonaTestResult {
  ok: boolean;
  message: string;
  reply?: string | null;
}

export interface ReportGenerateResult {
  id: number;
  title: string;
  content: string;
  used_ai: boolean;
  template_id: string;
  date_from: string;
  date_to: string;
}

export interface GeneratedReport {
  id: number;
  template_id: string;
  title: string;
  date_from: string;
  date_to: string;
  content: string;
  used_ai: boolean;
  created_at: string;
}

export interface TimelineAiEntry {
  cache_key: string;
  started_at: string;
  work_type: string;
  summary: string;
  used_ai: boolean;
}

export interface TimelineDescribeChunkEvent {
  offset: number;
  limit: number;
  entries: TimelineAiEntry[];
}

export const xiaohan = {
  ping: () => invoke<string>("app_ping"),
  getDataPath: () => invoke<string>("app_get_data_path"),
  getPromptsPath: () => invoke<string>("app_get_prompts_path"),
  getVendorsConfigPath: () => invoke<string>("app_get_vendors_config_path"),
  getPersonasPath: () => invoke<string>("app_get_personas_path"),
  personaList: () => invoke<PersonaInfo[]>("persona_list"),
  personaSetActive: (personaId: string) =>
    invoke<void>("persona_set_active", { personaId }),
  personaGetDetail: (personaId: string) =>
    invoke<PersonaDetail>("persona_get_detail", { personaId }),
  personaImport: (files: PersonaImportFile[]) =>
    invoke<PersonaImportResult>("persona_import", { files }),
  personaImportText: (args: {
    personaId?: string | null;
    id?: string | null;
    name?: string | null;
    text: string;
  }) =>
    invoke<PersonaImportResult>("persona_import_text", {
      personaId: args.personaId ?? null,
      id: args.id ?? null,
      name: args.name ?? null,
      text: args.text,
    }),
  personaImportWiki: (args: {
    url: string;
    personaId?: string | null;
    id?: string | null;
    name?: string | null;
  }) =>
    invoke<PersonaImportResult>("persona_import_wiki", {
      url: args.url,
      personaId: args.personaId ?? null,
      id: args.id ?? null,
      name: args.name ?? null,
    }),
  personaUpdate: (personaId: string, input: PersonaUpdateInput) =>
    invoke<void>("persona_update", { personaId, input }),
  personaDelete: (personaId: string) => invoke<void>("persona_delete", { personaId }),
  aiTestPersona: () => invoke<PersonaTestResult>("ai_test_persona"),
  getStatus: () => invoke<StatusPayload>("tracking_get_status"),
  setEnabled: (enabled: boolean) => invoke<void>("tracking_set_enabled", { enabled }),
  getSetting: (key: string) => invoke<string | null>("settings_get", { key }),
  saveSetting: (key: string, value: string) => invoke<void>("settings_save", { key, value }),
  autostartGetStatus: () => invoke<AutostartStatus>("autostart_get_status"),
  autostartSetEnabled: (enabled: boolean) =>
    invoke<void>("autostart_set_enabled", { enabled }),
  getOverview: () => invoke<OverviewPayload>("stats_today_overview"),
  getAppBreakdown: () => invoke<AppBreakdownItem[]>("stats_app_breakdown"),
  getHourlyActivity: () => invoke<number[]>("stats_hourly_activity"),
  getThreeDayHeatmap: () => invoke<HeatmapDay[]>("stats_three_day_heatmap"),
  getTimeline: (limit = 50, offset = 0, sinceMinutes?: number) =>
    invoke<TimelinePage>("stats_timeline", {
      limit,
      offset,
      sinceMinutes: sinceMinutes ?? null,
    }),
  timelineCached: (limit = 50, offset = 0, date?: string, sinceMinutes?: number) =>
    invoke<TimelineAiEntry[]>("timeline_cached", {
      limit,
      offset,
      date,
      sinceMinutes: sinceMinutes ?? null,
    }),
  timelineDescribe: (limit = 50, offset = 0, date?: string, sinceMinutes?: number) =>
    invoke<TimelineAiEntry[]>("timeline_describe", {
      limit,
      offset,
      date,
      sinceMinutes: sinceMinutes ?? null,
    }),
  getTimelineAiLogsPath: () => invoke<string>("app_get_timeline_ai_logs_path"),
  vaultGetStatus: () => invoke<VaultStatus>("vault_get_status"),
  vaultSetup: (password?: string) => invoke<void>("vault_setup", { password: password ?? null }),
  vaultUnlock: (password?: string) => invoke<void>("vault_unlock", { password: password ?? null }),
  vaultLock: () => invoke<void>("vault_lock"),
  vaultList: () => invoke<VaultEntry[]>("vault_list_entries"),
  vaultAdd: (entry: VaultEntryInput) => invoke<number>("vault_add_entry", { entry }),
  vaultUpdate: (id: number, entry: VaultEntryInput) =>
    invoke<void>("vault_update_entry", { id, entry }),
  vaultDelete: (id: number) => invoke<void>("vault_delete_entry", { id }),
  vaultGetSecret: (id: number) => invoke<string>("vault_get_secret", { id }),
  analysisGetStatus: () => invoke<AnalysisStats>("analysis_get_status"),
  analysisListInsights: (limit = 30) =>
    invoke<ActivityInsight[]>("analysis_list_insights", { limit }),
  getTodayMetrics: () => invoke<DailyMetrics>("stats_today_metrics"),
  aiGetConfig: () => invoke<AiConfig>("ai_get_config"),
  aiIsTextReady: () => invoke<boolean>("ai_is_text_ready"),
  aiSaveConfig: (config: AiConfig) => invoke<void>("ai_save_config", { config }),
  aiListModels: (vendorId: string, kind: "text" | "vision" | "thinking") =>
    invoke<AiModelOption[]>("ai_list_models", { vendorId, kind }),
  aiImportModels: (vendorId: string, kind: "text" | "vision" | "thinking") =>
    invoke<AiModelOption[]>("ai_import_models", { vendorId, kind }),
  aiTestVendor: (vendorId: string) =>
    invoke<VendorTestResult>("ai_test_vendor", { vendorId }),
  aiAddCustomModel: (
    vendorId: string,
    kind: "text" | "vision" | "thinking",
    id: string,
    name: string,
  ) => invoke<void>("ai_add_custom_model", { vendorId, kind, id, name }),
  characterList: () => invoke<CharacterProfile[]>("character_list"),
  characterGet: (id: number) => invoke<CharacterProfile>("character_get", { id }),
  characterCreate: (name: string, source: string, rawText: string) =>
    invoke<CharacterProfile>("character_create", { name, source, rawText }),
  characterUpdateRaw: (id: number, rawText: string) =>
    invoke<CharacterProfile>("character_update_raw", { id, rawText }),
  characterUpdateJson: (id: number, profileJson: CharacterProfileData) =>
    invoke<CharacterProfile>("character_update_json", { id, profileJson }),
  characterSaveSkill: (id: number, skillMd: string) =>
    invoke<CharacterProfile>("character_save_skill", { id, skillMd }),
  characterDelete: (id: number) => invoke<void>("character_delete", { id }),
  characterPreprocess: (id: number) =>
    invoke<CharacterOpResult>("character_preprocess", { id }),
  characterMergeText: (id: number, text: string) =>
    invoke<CharacterOpResult>("character_merge_text", { id, text }),
  characterGenerateSkill: (id: number) =>
    invoke<CharacterOpResult>("character_generate_skill", { id }),
  characterApplyPersona: (id: number, activate = true) =>
    invoke<CharacterOpResult>("character_apply_persona", { id, activate }),
  workTypesGet: () => invoke<WorkTypeConfig>("work_types_get"),
  workTypesSave: (config: WorkTypeConfig) => invoke<void>("work_types_save", { config }),
  periodListSummaries: (limit = 20) =>
    invoke<PeriodSummary[]>("period_list_summaries", { limit }),
  reportGenerate: (templateId: string, dateFrom: string, dateTo: string) =>
    invoke<ReportGenerateResult>("report_generate", {
      templateId,
      dateFrom,
      dateTo,
    }),
  reportList: (limit = 50) => invoke<GeneratedReport[]>("report_list", { limit }),
  reportDelete: (id: number) => invoke<void>("report_delete", { id }),
  petGetStatus: () => invoke<PetStatus>("pet_get_status"),
  petGetConfig: () => invoke<PetConfig>("pet_get_config"),
  petListModels: () => invoke<PetModelInfo[]>("pet_list_models"),
  petSetModel: (modelId: string) => invoke<void>("pet_set_model", { modelId }),
  petSaveModelSettings: (
    modelId: string,
    settings: {
      powerMode?: string;
      scale?: number;
      remarkIntervalSec?: number;
    },
  ) =>
    invoke<void>("pet_save_model_settings", {
      modelId,
      powerMode: settings.powerMode ?? null,
      scale: settings.scale ?? null,
      remarkIntervalSec: settings.remarkIntervalSec ?? null,
    }),
  petImportFromFolder: (name: string, folder: string) =>
    invoke<PetModelInfo>("pet_import_from_folder", { name, folder }),
  petImportFiles: (payload: PetImportFilesPayload) =>
    invoke<PetModelInfo>("pet_import_files", { payload }),
  petPickModelFolder: () => invoke<string | null>("pet_pick_model_folder"),
  petStageFolderImport: (folder: string) =>
    invoke<PetImportStagingPreview>("pet_stage_folder_import", { folder }),
  petStageFilesImport: (payload: PetStageFilesPayload) =>
    invoke<PetImportStagingPreview>("pet_stage_files_import", { payload }),
  petGetImportStaging: () =>
    invoke<PetImportStagingPreview | null>("pet_get_import_staging"),
  petClearImportStaging: () => invoke<void>("pet_clear_import_staging"),
  petCommitImport: (name: string) => invoke<PetModelInfo>("pet_commit_import", { name }),
  petDeleteModel: (modelId: string) => invoke<void>("pet_delete_model", { modelId }),
  petShow: () => invoke<void>("pet_show"),
  petHide: (destroy = false) => invoke<void>("pet_hide", { destroy }),
  petSetEnabled: (enabled: boolean) => invoke<void>("pet_set_enabled", { enabled }),
  petOpenMain: () => invoke<void>("pet_open_main"),
  petReload: () => invoke<void>("pet_reload"),
  petNudge: () => invoke<void>("pet_nudge"),
  petRefreshAnimations: () => invoke<void>("pet_refresh_animations"),
  petPreviewAnimation: (animation: string, loopAnim?: boolean) =>
    invoke<void>("pet_preview_animation", { animation, loopAnim }),
  petSyncAnimations: (payload: {
    model_id: string;
    animations: string[];
    idle_animation?: string | null;
  }) => invoke<PetAnimationMeta>("pet_sync_animations", { payload }),
  petSetIdleAnimation: (modelId: string, idleAnimation: string) =>
    invoke<PetAnimationMeta>("pet_set_idle_animation", { modelId, idleAnimation }),
  petSetClickAnimation: (modelId: string, clickAnimation: string) =>
    invoke<PetAnimationMeta>("pet_set_click_animation", { modelId, clickAnimation }),
  petSetRandomAnimations: (payload: {
    model_id: string;
    animations: string[];
    min_sec: number;
    max_sec: number;
  }) => invoke<PetAnimationMeta>("pet_set_random_animations", { payload }),
  petSaveAnimationLayout: (payload: {
    model_id: string;
    idle_animation?: string | null;
    click_animation?: string | null;
    boot_animation?: string | null;
    return_idle_animation?: string | null;
  drag_animation?: string | null;
    random_animations: string[];
    random_min_sec: number;
    random_max_sec: number;
    lines: PetRemarkLine[];
  }) => invoke<PetAnimationMeta>("pet_save_animation_layout", { payload }),
  petImportLines: (payload: {
    model_id: string;
    lines: PetRemarkLine[];
    append?: boolean;
  }) => invoke<PetAnimationMeta>("pet_import_lines", { payload }),
  petAiSuggestLines: (modelId: string, count?: number) =>
    invoke<PetRemarkLine[]>("pet_ai_suggest_lines", { modelId, count }),
  petAiImportLines: (modelId: string, rawText: string) =>
    invoke<PetRemarkLine[]>("pet_ai_import_lines", { modelId, rawText }),
  petWikiImportLines: (modelId: string, url: string) =>
    invoke<PetRemarkLine[]>("pet_wiki_import_lines", { modelId, url }),
  petSaveLayout: (
    width: number,
    height: number,
    scale: number,
    offsetX: number,
    offsetY: number,
  ) => invoke<void>("pet_save_layout", { width, height, scale, offsetX, offsetY }),
  getPerformanceSnapshot: () => invoke<PerformanceSnapshot>("system_get_performance"),
};

export function formatDuration(ms: number): string {
  const totalSec = Math.floor(ms / 1000);
  const h = Math.floor(totalSec / 3600);
  const m = Math.floor((totalSec % 3600) / 60);
  const s = totalSec % 60;
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
}

export function formatHours(ms: number): string {
  const h = ms / 3_600_000;
  if (h >= 1) return `${h.toFixed(1)}h`;
  const m = Math.round(ms / 60_000);
  return m > 0 ? `${m}m` : "";
}
