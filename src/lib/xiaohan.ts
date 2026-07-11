import { tauriInvoke as invoke } from "./tauriInvoke";

// ── 桌宠 / 性能 ──

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

export interface AutostartStatus {
  enabled: boolean;
  supported: boolean;
}

export interface McpApiStatus {
  enabled: boolean;
}

export interface PetRemarkLine {
  text: string;
  animation?: string | null;
}

export interface PetStatus {
  enabled: boolean;
  visible: boolean;
  /** 设置页「启用桌宠」：已启用且非用户主动隐藏 */
  active: boolean;
  power_mode: string;
  scale: number;
  remark_interval_sec: number;
  bubble_enabled: boolean;
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

export interface PetStatusChangedPayload {
  active: boolean;
  bubble_enabled: boolean;
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

export interface PetImportStagingPreview {
  source: string;
  folder_path?: string | null;
  skel_file: string;
  atlas_file: string;
  png_file: string;
  config_file?: string | null;
  config_generated: boolean;
}

export interface PetStageFilesPayload {
  skel_b64: string;
  atlas_b64: string;
  png_b64: string;
  skel_name?: string;
  atlas_name?: string;
  png_name?: string;
}

// ── 人物 / 人设 ──

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

export interface PersonaInfo {
  id: string;
  name: string;
  source: string;
  description: string;
  active: boolean;
  has_profile: boolean;
  is_builtin: boolean;
}

export interface CharacterSkinInfo {
  id: string;
  name: string;
  model_id: string;
  model_name: string;
  active: boolean;
  model_ready: boolean;
}

export interface CharacterBrief {
  id: string;
  name: string;
  source: string;
  description: string;
  persona_id: string;
  active: boolean;
  active_skin_id: string;
  active_skin_name: string;
  skin_count: number;
  is_builtin: boolean;
  faction: string;
  ship_type: string;
  rarity: string;
  trait_summary: string;
  avatar_path: string | null;
  avatar_url: string | null;
}

export interface CharacterListPage {
  total: number;
  offset: number;
  limit: number;
  items: CharacterBrief[];
}

export interface CharacterSkinsPage {
  total: number;
  offset: number;
  limit: number;
  active_skin_id: string;
  items: CharacterSkinInfo[];
}

export interface CharacterDetail {
  id: string;
  name: string;
  source: string;
  description: string;
  persona_id: string;
  active: boolean;
  active_skin_id: string;
  active_skin_name: string;
  active_model_id: string;
  active_model_name: string;
  active_model_ready: boolean;
  skin_count: number;
  is_builtin: boolean;
  faction: string;
  ship_type: string;
  rarity: string;
  trait_summary: string;
  avatar_path: string | null;
  avatar_url: string | null;
  skill_md: string;
  profile_json: CharacterProfileData;
  has_profile: boolean;
  profile_ai_updated: boolean;
  profile_ai_updated_at: string | null;
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
  profile_ai_updated: boolean;
  profile_ai_updated_at: string | null;
}

export interface PersonaImportFile {
  filename: string;
  content: string;
}

export interface PersonaImportResult {
  imported_ids: string[];
  message: string;
}

export interface CharacterWikiImportResult {
  message: string;
  lines_imported: number;
  persona_id: string;
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

export interface PetWikiBulkImportStartResult {
  started: boolean;
  already_running: boolean;
}

export interface PetWikiBulkImportProgress {
  phase: string;
  index: number;
  total: number;
  model_id: string;
  model_name: string;
  message: string;
  lines_imported: number;
  succeeded: number;
  failed: number;
  skipped: number;
  updated_at_ms: number;
}

export interface PersonaUpdateInput {
  name: string;
  source: string;
  description: string;
  skill_md: string;
  profile_json: CharacterProfileData;
}

/** [live2d-only] 纯桌宠 IPC 子集 */
export const xiaohan = {
  ping: () => invoke<string>("app_ping"),
  getDataPath: () => invoke<string>("app_get_data_path"),
  getSetting: (key: string) => invoke<string | null>("settings_get", { key }),
  saveSetting: (key: string, value: string) => invoke<void>("settings_save", { key, value }),
  autostartGetStatus: () => invoke<AutostartStatus>("autostart_get_status"),
  autostartSetEnabled: (enabled: boolean) =>
    invoke<void>("autostart_set_enabled", { enabled }),
  mcpApiGetStatus: () => invoke<McpApiStatus>("mcp_api_get_status"),
  mcpApiSetEnabled: (enabled: boolean) =>
    invoke<void>("mcp_api_set_enabled", { enabled }),
  getPerformanceSnapshot: () => invoke<PerformanceSnapshot>("system_get_performance"),

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
    url?: string | null;
    wikiTitle?: string | null;
    personaId?: string | null;
    id?: string | null;
    name?: string | null;
  }) =>
    invoke<PersonaImportResult>("persona_import_wiki", {
      url: args.url ?? null,
      wikiTitle: args.wikiTitle ?? null,
      personaId: args.personaId ?? null,
      id: args.id ?? null,
      name: args.name ?? null,
    }),
  personaImportBlhxLocal: (args: {
    wikiTitle: string;
    personaId?: string | null;
    id?: string | null;
    name?: string | null;
  }) =>
    invoke<PersonaImportResult>("persona_import_blhx_local", {
      wikiTitle: args.wikiTitle,
      personaId: args.personaId ?? null,
      id: args.id ?? null,
      name: args.name ?? null,
    }),
  personaUpdate: (personaId: string, input: PersonaUpdateInput) =>
    invoke<void>("persona_update", { personaId, input }),
  personaDelete: (personaId: string) => invoke<void>("persona_delete", { personaId }),

  charactersImportWiki: (args: {
    characterId: string;
    wikiTitle?: string | null;
    url?: string | null;
  }) =>
    invoke<CharacterWikiImportResult>("character_import_wiki", {
      characterId: args.characterId,
      wikiTitle: args.wikiTitle ?? null,
      url: args.url ?? null,
    }),
  charactersListPage: (args: {
    offset: number;
    limit: number;
    query?: string;
    favoritesOnly?: boolean;
    favoriteIds?: string[];
  }) => {
    const payload: Record<string, unknown> = {
      offset: args.offset,
      limit: args.limit,
    };
    if (args.query?.trim()) payload.query = args.query.trim();
    if (args.favoritesOnly) {
      payload.favoritesOnly = true;
      if (args.favoriteIds?.length) payload.favoriteIds = args.favoriteIds;
    }
    return invoke<CharacterListPage>("characters_list_page", payload);
  },
  charactersGetDetail: (characterId: string) =>
    invoke<CharacterDetail>("characters_get_detail", { characterId }),
  charactersCacheAvatar: (characterId: string) =>
    invoke<string | null>("characters_cache_avatar", { characterId }),
  charactersReadAvatar: (characterId: string) =>
    invoke<string | null>("characters_read_avatar", { characterId }),
  charactersSkinsPage: (characterId: string, offset = 0, limit = 12) =>
    invoke<CharacterSkinsPage>("characters_skins_page", { characterId, offset, limit }),
  charactersSetActive: (characterId: string) =>
    invoke<void>("characters_set_active", { characterId }),
  charactersSetSkin: (characterId: string, skinId: string) =>
    invoke<void>("characters_set_skin", { characterId, skinId }),
  charactersRemoveSkin: (
    characterId: string,
    skinId: string,
    deleteModelFiles = true,
  ) =>
    invoke<void>("characters_remove_skin", {
      characterId,
      skinId,
      deleteModelFiles,
    }),

  petGetStatus: () => invoke<PetStatus>("pet_get_status"),
  petGetWikiBulkImportProgress: () =>
    invoke<PetWikiBulkImportProgress | null>("pet_get_wiki_bulk_import_progress"),
  petStartWikiBulkImport: () =>
    invoke<PetWikiBulkImportStartResult>("pet_start_wiki_bulk_import"),
  petPauseWikiBulkImport: () => invoke<boolean>("pet_pause_wiki_bulk_import"),
  petResumeWikiBulkImport: () => invoke<boolean>("pet_resume_wiki_bulk_import"),
  petStopWikiBulkImport: () => invoke<boolean>("pet_stop_wiki_bulk_import"),
  petGetModelStatus: (modelId: string) =>
    invoke<PetStatus>("pet_get_model_status", { modelId }),
  petSaveModelSettings: (
    modelId: string,
    settings: {
      powerMode?: string;
      remarkIntervalSec?: number;
      applyLive?: boolean;
    },
  ) =>
    invoke<void>("pet_save_model_settings", {
      modelId,
      powerMode: settings.powerMode ?? null,
      remarkIntervalSec: settings.remarkIntervalSec ?? null,
      applyLive: settings.applyLive ?? null,
    }),
  petSetScale: (scale: number) => invoke<void>("pet_set_scale", { scale }),
  petSetEnabled: (enabled: boolean) => invoke<void>("pet_set_enabled", { enabled }),
  petGetBubbleEnabled: () => invoke<boolean>("pet_get_bubble_enabled"),
  petSetBubbleEnabled: (enabled: boolean) =>
    invoke<void>("pet_set_bubble_enabled", { enabled }),
  petPickModelFolder: () => invoke<string | null>("pet_pick_model_folder"),
  petStageFolderImport: (folder: string) =>
    invoke<PetImportStagingPreview>("pet_stage_folder_import", { folder }),
  petStageFilesImport: (payload: PetStageFilesPayload) =>
    invoke<PetImportStagingPreview>("pet_stage_files_import", { payload }),
  petGetImportStaging: () =>
    invoke<PetImportStagingPreview | null>("pet_get_import_staging"),
  petCommitImport: (name: string, characterId?: string) =>
    invoke<PetModelInfo>("pet_commit_import", { name, characterId: characterId ?? null }),
  petClearImportStaging: () => invoke<void>("pet_clear_import_staging"),
  petDeleteModel: (modelId: string) => invoke<void>("pet_delete_model", { modelId }),
  petRefreshAnimations: () => invoke<void>("pet_refresh_animations"),
  petPreviewAnimation: (animation: string, loopAnim?: boolean) =>
    invoke<void>("pet_preview_animation", { animation, loopAnim }),
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
  petWikiImportLines: (modelId: string, url: string) =>
    invoke<PetRemarkLine[]>("pet_wiki_import_lines", { modelId, url }),
};
