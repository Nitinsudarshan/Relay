import {
  KnowledgeNode,
  KnowledgeEdge,
  GraphFiltersSettings,
  GraphGroup,
  GraphDisplaySettings,
  GraphForcesSettings,
  LocalGraphSettings,
} from '../../../types';

export interface SimNode extends KnowledgeNode {
  x: number;
  y: number;
  vx: number;
  vy: number;
  radius: number;
  color: string;
  isPinned?: boolean;
  opacity?: number;
  createdAtTimestamp?: number;
}

export interface SimEdge extends KnowledgeEdge {
  isExplicit?: boolean;
}

export interface CameraState {
  x: number;
  y: number;
  k: number;
}

export const RELAY_COLOR_MAP: Record<string, string> = {
  scribble: '#3b82f6',     // Electric blue (Scribble)
  voice_note: '#ec4899',   // Vibrant pink (Voice Note)
  topic: '#f59e0b',        // Warm amber (Topic cluster)
  entity: '#10b981',       // Emerald green (Named Entity)
  person: '#8b5cf6',       // Purple (Person)
  organization: '#14b8a6', // Teal (Organization)
  place: '#f97316',        // Orange (Place)
  project: '#06b6d4',      // Cyan (Project)
  source: '#64748b',       // Slate (Source/Attachment)
  file: '#64748b',         // Slate (File)
  document: '#0284c7',     // Sky (Document)
  task: '#84cc16',         // Lime (Task)
  meeting: '#a855f7',      // Fuchsia (Meeting)
  unresolved: '#6b7280',   // Gray (Unresolved)
  default: '#94a3b8',      // Slate default
};

export const PRESET_GROUP_COLORS = [
  '#ef4444', // Red
  '#f97316', // Orange
  '#eab308', // Yellow
  '#22c55e', // Green
  '#06b6d4', // Cyan
  '#3b82f6', // Blue
  '#8b5cf6', // Purple
  '#ec4899', // Pink
  '#14b8a6', // Teal
  '#f43f5e', // Rose
];

export const DEFAULT_FILTERS: GraphFiltersSettings = {
  searchQuery: '',
  showScribbles: true,
  showVoiceNotes: true,
  showTags: true, // Topics
  showEntities: true,
  showAttachments: true,
  existingFilesOnly: false,
  showOrphans: true,
  showUnresolved: true,
};

export const DEFAULT_DISPLAY: GraphDisplaySettings = {
  showArrows: false,
  textFadeThreshold: 0.6,
  nodeSizeMultiplier: 1.0,
  linkThickness: 1.0,
};

export const DEFAULT_FORCES: GraphForcesSettings = {
  centerForce: 0.40,
  repelForce: 10.0,
  linkForce: 0.60,
  linkDistance: 130,
};

export const DEFAULT_LOCAL_GRAPH: LocalGraphSettings = {
  enabled: false,
  rootNodeId: null,
  depth: 1,
};
