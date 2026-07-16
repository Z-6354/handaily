export interface CatalogEntry {
  wikiTitle: string;
  wikiPath: string;
  displayName: string;
  aliases: string[];
  avatarUrl: string | null;
  rarity: string | null;
  faction: string | null;
  shipType: string | null;
}

export interface CharacterInfoField {
  field: string;
  value: string;
}

export interface ShipLine {
  key: string;
  label: string | null;
  lang: string;
  text: string;
  audioUrl: string | null;
  /** Present when flattened from a by-skin group. */
  skin?: string;
}

export type ShipLineSkinKind =
  | "default"
  | "skin"
  | "retrofit"
  | "oath"
  | "other";

/** One illustration-tab skin (TabContainer), before lines are bound. */
export interface ShipSkinSlot {
  key: string;
  label: string;
  kind: "default" | "skin" | "oath";
  image_url: string | null;
  image_alt: string | null;
  sort_order: number;
}

/** One Wiki 台词 panel (通常 / 换装名 / 改造 / 誓约). */
export interface ShipLineGroup {
  skin: string;
  skin_kind: ShipLineSkinKind;
  lines: ShipLine[];
  /** Matches ShipSkinSlot.key when bound. */
  slot_key?: string;
}

export interface ShipAsset {
  kind: "avatar" | "illustration" | "skin" | "chibi" | "other";
  name: string;
  url: string;
}

export interface ShipSection {
  id: string;
  title: string;
  text: string;
}

export interface ShipRecord {
  wikiTitle: string;
  wikiUrl: string;
  displayName: string;
  aliases: string[];
  rarity: string | null;
  faction: string | null;
  shipType: string | null;
  cv: string | null;
  characterInfo: CharacterInfoField[];
  sections: ShipSection[];
  lines: ShipLine[];
  /** Per-panel lines when scraped; empty if legacy-only. */
  linesBySkin: ShipLineGroup[];
  /** Illustration TabContainer skins (改造 excluded; empty tabs dropped). */
  skins: ShipSkinSlot[];
  assets: ShipAsset[];
  personaReference: string;
  fetchedAt: string;
  htmlHash: string;
}

export interface SyncStats {
  catalogTotal: number;
  fetchedTotal: number;
  pendingTotal: number;
  lastCatalogSync: string | null;
}

export interface HandailyExport {
  wikiTitle: string;
  wikiUrl: string;
  name: string;
  source: string;
  personaReference: string;
  sampleLines: string[];
  characterInfo: Record<string, string>;
  sections: Record<string, string>;
  assets: ShipAsset[];
  suggestedPersonaId: string;
}
