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
