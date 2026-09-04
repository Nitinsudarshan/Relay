import {
  GraphFiltersSettings,
  GraphGroup,
  GraphDisplaySettings,
  GraphForcesSettings,
  GraphPositionMap,
} from '../../../types';
import {
  DEFAULT_FILTERS,
  DEFAULT_DISPLAY,
  DEFAULT_FORCES,
} from './graphTypes';

const SETTINGS_STORAGE_KEY = 'relay_knowledge_graph_settings_v2';
const POSITIONS_STORAGE_KEY = 'relay_knowledge_graph_positions_v1';

export interface StoredGraphSettings {
  filters: GraphFiltersSettings;
  groups: GraphGroup[];
  display: GraphDisplaySettings;
  forces: GraphForcesSettings;
}

/**
 * Load persisted graph settings from LocalStorage
 */
export function loadGraphSettings(): StoredGraphSettings {
  try {
    const raw = localStorage.getItem(SETTINGS_STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      return {
        filters: { ...DEFAULT_FILTERS, ...(parsed.filters || {}) },
        groups: Array.isArray(parsed.groups) ? parsed.groups : [],
        display: { ...DEFAULT_DISPLAY, ...(parsed.display || {}) },
        forces: { ...DEFAULT_FORCES, ...(parsed.forces || {}) },
      };
    }
  } catch (err) {
    console.warn('Failed to parse saved graph settings:', err);
  }

  return {
    filters: DEFAULT_FILTERS,
    groups: [],
    display: DEFAULT_DISPLAY,
    forces: DEFAULT_FORCES,
  };
}

/**
 * Save graph settings to LocalStorage
 */
export function saveGraphSettings(settings: StoredGraphSettings): boolean {
  try {
    localStorage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(settings));
    return true;
  } catch (err) {
    console.error('Failed to save graph settings:', err);
    return false;
  }
}

/**
 * Load persisted node coordinates
 */
export function loadNodePositions(): GraphPositionMap {
  try {
    const raw = localStorage.getItem(POSITIONS_STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (parsed && typeof parsed === 'object') {
        return parsed;
      }
    }
  } catch (err) {
    console.warn('Failed to parse saved node positions:', err);
  }
  return {};
}

/**
 * Save node coordinates to LocalStorage
 */
export function saveNodePositions(positions: GraphPositionMap): boolean {
  try {
    localStorage.setItem(POSITIONS_STORAGE_KEY, JSON.stringify(positions));
    return true;
  } catch (err) {
    console.error('Failed to save node positions:', err);
    return false;
  }
}

/**
 * Reset node positions (clears saved coordinates)
 */
export function clearNodePositions(): void {
  try {
    localStorage.removeItem(POSITIONS_STORAGE_KEY);
  } catch (err) {
    console.error('Failed to clear node positions:', err);
  }
}
