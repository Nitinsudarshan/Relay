import React, { useState } from 'react';
import {
  GraphFiltersSettings,
  GraphGroup,
  GraphDisplaySettings,
  GraphForcesSettings,
} from '../../../types';
import {
  Search,
  X,
  Plus,
  Play,
  RotateCcw,
  ChevronDown,
  ChevronRight,
  Save,
  Check,
  LayoutGrid,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { PRESET_GROUP_COLORS } from './graphTypes';

interface GraphSettingsPanelProps {
  isOpen: boolean;
  onClose: () => void;
  filters: GraphFiltersSettings;
  onFiltersChange: (filters: GraphFiltersSettings) => void;
  groups: GraphGroup[];
  onGroupsChange: (groups: GraphGroup[]) => void;
  display: GraphDisplaySettings;
  onDisplayChange: (display: GraphDisplaySettings) => void;
  forces: GraphForcesSettings;
  onForcesChange: (forces: GraphForcesSettings) => void;
  onSaveSettings: () => void;
  onResetSettings: () => void;
  onResetLayout: () => void;
  onReheatSimulation: () => void;
  saveFeedback: boolean;
}

export const GraphSettingsPanel: React.FC<GraphSettingsPanelProps> = ({
  isOpen,
  onClose,
  filters,
  onFiltersChange,
  groups,
  onGroupsChange,
  display,
  onDisplayChange,
  forces,
  onForcesChange,
  onSaveSettings,
  onResetSettings,
  onResetLayout,
  onReheatSimulation,
  saveFeedback,
}) => {
  const [collapsedSections, setCollapsedSections] = useState<Record<string, boolean>>({
    filters: false,
    groups: true,
    display: true,
    forces: true,
  });

  if (!isOpen) return null;

  const toggleSection = (section: string) => {
    setCollapsedSections((prev) => ({ ...prev, [section]: !prev[section] }));
  };

  const handleAddGroup = () => {
    const nextColor = PRESET_GROUP_COLORS[groups.length % PRESET_GROUP_COLORS.length];
    const newGroup: GraphGroup = {
      id: `grp_${Date.now()}`,
      query: '',
      color: nextColor,
    };
    onGroupsChange([...groups, newGroup]);
  };

  const handleUpdateGroup = (id: string, updates: Partial<GraphGroup>) => {
    onGroupsChange(groups.map((g) => (g.id === id ? { ...g, ...updates } : g)));
  };

  const handleRemoveGroup = (id: string) => {
    onGroupsChange(groups.filter((g) => g.id !== id));
  };

  return (
    <div className="absolute top-16 right-4 w-76 max-h-[calc(100%-80px)] overflow-y-auto bg-card/95 backdrop-blur-xl border border-border rounded-lg shadow-2xl p-3.5 space-y-3 z-30 animate-in fade-in duration-150 font-sans text-xs">
      {/* Panel Header */}
      <div className="flex items-center justify-between pb-1.5 border-b border-border/50">
        <span className="font-bold text-foreground tracking-tight">Graph Settings</span>
        <button
          onClick={onClose}
          className="text-muted-foreground hover:text-foreground p-1 rounded-md transition-colors"
          title="Close Settings"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      </div>

      {/* 1. FILTERS SECTION */}
      <div className="border border-border/60 rounded-lg overflow-hidden bg-card/40">
        <button
          type="button"
          onClick={() => toggleSection('filters')}
          className="w-full p-2.5 flex items-center justify-between font-semibold text-foreground hover:bg-muted/30 transition-all text-left"
        >
          <span>Filters</span>
          {collapsedSections.filters ? <ChevronRight className="w-3.5 h-3.5 text-muted-foreground" /> : <ChevronDown className="w-3.5 h-3.5 text-muted-foreground" />}
        </button>

        {!collapsedSections.filters && (
          <div className="p-3 border-t border-border/50 space-y-3">
            {/* Search query input */}
            <div className="relative">
              <Search className="w-3.5 h-3.5 absolute left-2.5 top-2 text-muted-foreground" />
              <input
                type="text"
                value={filters.searchQuery}
                onChange={(e) => onFiltersChange({ ...filters, searchQuery: e.target.value })}
                placeholder="Search graph…"
                className="w-full pl-8 pr-2 py-1 text-xs bg-muted/30 border border-border/60 rounded-md text-foreground placeholder:text-muted-foreground/60 focus:outline-none focus:border-primary"
              />
            </div>

            {/* Filter Toggles */}
            <div className="space-y-2 text-[11px]">
              <label className="flex items-center justify-between text-muted-foreground hover:text-foreground cursor-pointer">
                <span>Scribbles</span>
                <input
                  type="checkbox"
                  checked={filters.showScribbles ?? true}
                  onChange={(e) => onFiltersChange({ ...filters, showScribbles: e.target.checked })}
                  className="rounded border-border text-primary focus:ring-0 cursor-pointer"
                />
              </label>

              <label className="flex items-center justify-between text-muted-foreground hover:text-foreground cursor-pointer">
                <span>Voice Notes</span>
                <input
                  type="checkbox"
                  checked={filters.showVoiceNotes ?? true}
                  onChange={(e) => onFiltersChange({ ...filters, showVoiceNotes: e.target.checked })}
                  className="rounded border-border text-primary focus:ring-0 cursor-pointer"
                />
              </label>

              <label className="flex items-center justify-between text-muted-foreground hover:text-foreground cursor-pointer">
                <span>Topics</span>
                <input
                  type="checkbox"
                  checked={filters.showTags}
                  onChange={(e) => onFiltersChange({ ...filters, showTags: e.target.checked })}
                  className="rounded border-border text-primary focus:ring-0 cursor-pointer"
                />
              </label>

              <label className="flex items-center justify-between text-muted-foreground hover:text-foreground cursor-pointer">
                <span>Entities & People</span>
                <input
                  type="checkbox"
                  checked={filters.showEntities ?? true}
                  onChange={(e) => onFiltersChange({ ...filters, showEntities: e.target.checked })}
                  className="rounded border-border text-primary focus:ring-0 cursor-pointer"
                />
              </label>

              <label className="flex items-center justify-between text-muted-foreground hover:text-foreground cursor-pointer">
                <span>Attachments & Sources</span>
                <input
                  type="checkbox"
                  checked={filters.showAttachments}
                  onChange={(e) => onFiltersChange({ ...filters, showAttachments: e.target.checked })}
                  className="rounded border-border text-primary focus:ring-0 cursor-pointer"
                />
              </label>

              <label className="flex items-center justify-between text-muted-foreground hover:text-foreground cursor-pointer">
                <span>Existing Scribbles only</span>
                <input
                  type="checkbox"
                  checked={filters.existingFilesOnly}
                  onChange={(e) => onFiltersChange({ ...filters, existingFilesOnly: e.target.checked })}
                  className="rounded border-border text-primary focus:ring-0 cursor-pointer"
                />
              </label>

              <label className="flex items-center justify-between text-muted-foreground hover:text-foreground cursor-pointer">
                <span>Orphans (Unconnected)</span>
                <input
                  type="checkbox"
                  checked={filters.showOrphans}
                  onChange={(e) => onFiltersChange({ ...filters, showOrphans: e.target.checked })}
                  className="rounded border-border text-primary focus:ring-0 cursor-pointer"
                />
              </label>
            </div>
          </div>
        )}
      </div>

      {/* 2. GROUPS SECTION */}
      <div className="border border-border/60 rounded-lg overflow-hidden bg-card/40">
        <button
          type="button"
          onClick={() => toggleSection('groups')}
          className="w-full p-2.5 flex items-center justify-between font-semibold text-foreground hover:bg-muted/30 transition-all text-left"
        >
          <span>Groups ({groups.length})</span>
          {collapsedSections.groups ? <ChevronRight className="w-3.5 h-3.5 text-muted-foreground" /> : <ChevronDown className="w-3.5 h-3.5 text-muted-foreground" />}
        </button>

        {!collapsedSections.groups && (
          <div className="p-3 border-t border-border/50 space-y-2.5">
            {groups.map((grp) => (
              <div key={grp.id} className="flex items-center gap-1.5">
                <input
                  type="text"
                  value={grp.query}
                  onChange={(e) => handleUpdateGroup(grp.id, { query: e.target.value })}
                  placeholder="Enter query…"
                  className="flex-1 px-2 py-1 text-xs bg-muted/30 border border-border/60 rounded-md text-foreground placeholder:text-muted-foreground/60 focus:outline-none focus:border-primary"
                />
                <input
                  type="color"
                  value={grp.color}
                  onChange={(e) => handleUpdateGroup(grp.id, { color: e.target.value })}
                  className="w-6 h-6 rounded-md cursor-pointer border-0 bg-transparent shrink-0"
                  title="Group color"
                />
                <button
                  type="button"
                  onClick={() => handleRemoveGroup(grp.id)}
                  className="text-muted-foreground hover:text-destructive p-1 rounded-md"
                >
                  <X className="w-3.5 h-3.5" />
                </button>
              </div>
            ))}

            <Button
              size="sm"
              variant="outline"
              onClick={handleAddGroup}
              className="w-full h-7 text-[11px] gap-1 bg-muted/20 hover:bg-muted/40"
            >
              <Plus className="w-3.5 h-3.5" />
              <span>New group</span>
            </Button>
          </div>
        )}
      </div>

      {/* 3. DISPLAY SECTION */}
      <div className="border border-border/60 rounded-lg overflow-hidden bg-card/40">
        <button
          type="button"
          onClick={() => toggleSection('display')}
          className="w-full p-2.5 flex items-center justify-between font-semibold text-foreground hover:bg-muted/30 transition-all text-left"
        >
          <span>Display</span>
          {collapsedSections.display ? <ChevronRight className="w-3.5 h-3.5 text-muted-foreground" /> : <ChevronDown className="w-3.5 h-3.5 text-muted-foreground" />}
        </button>

        {!collapsedSections.display && (
          <div className="p-3 border-t border-border/50 space-y-3 text-[11px]">
            {/* Arrows Toggle */}
            <label className="flex items-center justify-between text-muted-foreground hover:text-foreground cursor-pointer">
              <span>Arrows</span>
              <input
                type="checkbox"
                checked={display.showArrows}
                onChange={(e) => onDisplayChange({ ...display, showArrows: e.target.checked })}
                className="rounded border-border text-primary focus:ring-0 cursor-pointer"
              />
            </label>

            {/* Text fade threshold */}
            <div className="space-y-1">
              <div className="flex items-center justify-between text-muted-foreground">
                <span>Text fade threshold</span>
                <span className="font-mono">{display.textFadeThreshold.toFixed(2)}</span>
              </div>
              <input
                type="range"
                min="0.00"
                max="2.00"
                step="0.05"
                value={display.textFadeThreshold}
                onChange={(e) => onDisplayChange({ ...display, textFadeThreshold: parseFloat(e.target.value) })}
                className="w-full h-1 bg-muted rounded appearance-none cursor-pointer accent-primary"
              />
            </div>

            {/* Node size */}
            <div className="space-y-1">
              <div className="flex items-center justify-between text-muted-foreground">
                <span>Node size</span>
                <span className="font-mono">{display.nodeSizeMultiplier.toFixed(2)}</span>
              </div>
              <input
                type="range"
                min="0.50"
                max="3.00"
                step="0.10"
                value={display.nodeSizeMultiplier}
                onChange={(e) => onDisplayChange({ ...display, nodeSizeMultiplier: parseFloat(e.target.value) })}
                className="w-full h-1 bg-muted rounded appearance-none cursor-pointer accent-primary"
              />
            </div>

            {/* Link thickness */}
            <div className="space-y-1">
              <div className="flex items-center justify-between text-muted-foreground">
                <span>Link thickness</span>
                <span className="font-mono">{display.linkThickness.toFixed(2)}</span>
              </div>
              <input
                type="range"
                min="0.50"
                max="3.00"
                step="0.10"
                value={display.linkThickness}
                onChange={(e) => onDisplayChange({ ...display, linkThickness: parseFloat(e.target.value) })}
                className="w-full h-1 bg-muted rounded appearance-none cursor-pointer accent-primary"
              />
            </div>

            {/* Animate / Reheat Simulation Button */}
            <Button
              size="sm"
              variant="outline"
              onClick={onReheatSimulation}
              className="w-full h-7 text-[11px] gap-1.5 bg-muted/20 hover:bg-muted/40 mt-1"
            >
              <Play className="w-3.5 h-3.5" />
              <span>Animate / Settle</span>
            </Button>
          </div>
        )}
      </div>

      {/* 4. FORCES SECTION */}
      <div className="border border-border/60 rounded-lg overflow-hidden bg-card/40">
        <button
          type="button"
          onClick={() => toggleSection('forces')}
          className="w-full p-2.5 flex items-center justify-between font-semibold text-foreground hover:bg-muted/30 transition-all text-left"
        >
          <span>Forces</span>
          {collapsedSections.forces ? <ChevronRight className="w-3.5 h-3.5 text-muted-foreground" /> : <ChevronDown className="w-3.5 h-3.5 text-muted-foreground" />}
        </button>

        {!collapsedSections.forces && (
          <div className="p-3 border-t border-border/50 space-y-3 text-[11px]">
            {/* Center force */}
            <div className="space-y-1">
              <div className="flex items-center justify-between text-muted-foreground">
                <span>Center force</span>
                <span className="font-mono">{forces.centerForce.toFixed(2)}</span>
              </div>
              <input
                type="range"
                min="0.00"
                max="1.00"
                step="0.02"
                value={forces.centerForce}
                onChange={(e) => {
                  onForcesChange({ ...forces, centerForce: parseFloat(e.target.value) });
                  onReheatSimulation();
                }}
                className="w-full h-1 bg-muted rounded appearance-none cursor-pointer accent-primary"
              />
            </div>

            {/* Repel force */}
            <div className="space-y-1">
              <div className="flex items-center justify-between text-muted-foreground">
                <span>Repel force</span>
                <span className="font-mono">{forces.repelForce.toFixed(2)}</span>
              </div>
              <input
                type="range"
                min="0.00"
                max="20.00"
                step="0.50"
                value={forces.repelForce}
                onChange={(e) => {
                  onForcesChange({ ...forces, repelForce: parseFloat(e.target.value) });
                  onReheatSimulation();
                }}
                className="w-full h-1 bg-muted rounded appearance-none cursor-pointer accent-primary"
              />
            </div>

            {/* Link force */}
            <div className="space-y-1">
              <div className="flex items-center justify-between text-muted-foreground">
                <span>Link force</span>
                <span className="font-mono">{forces.linkForce.toFixed(2)}</span>
              </div>
              <input
                type="range"
                min="0.00"
                max="1.00"
                step="0.02"
                value={forces.linkForce}
                onChange={(e) => {
                  onForcesChange({ ...forces, linkForce: parseFloat(e.target.value) });
                  onReheatSimulation();
                }}
                className="w-full h-1 bg-muted rounded appearance-none cursor-pointer accent-primary"
              />
            </div>

            {/* Link distance */}
            <div className="space-y-1">
              <div className="flex items-center justify-between text-muted-foreground">
                <span>Link distance</span>
                <span className="font-mono">{forces.linkDistance}</span>
              </div>
              <input
                type="range"
                min="30"
                max="500"
                step="10"
                value={forces.linkDistance}
                onChange={(e) => {
                  onForcesChange({ ...forces, linkDistance: parseInt(e.target.value, 10) });
                  onReheatSimulation();
                }}
                className="w-full h-1 bg-muted rounded appearance-none cursor-pointer accent-primary"
              />
            </div>
          </div>
        )}
      </div>

      {/* Save & Reset Actions */}
      <div className="pt-2 border-t border-border/50 space-y-2">
        <Button
          size="sm"
          variant="default"
          onClick={onSaveSettings}
          className="w-full h-7 text-xs gap-1.5 bg-primary text-primary-foreground font-semibold shadow-xs"
        >
          {saveFeedback ? (
            <>
              <Check className="w-3.5 h-3.5 text-white" />
              <span>Settings Saved</span>
            </>
          ) : (
            <>
              <Save className="w-3.5 h-3.5" />
              <span>Save Graph Settings</span>
            </>
          )}
        </Button>

        <div className="flex items-center gap-1.5">
          <Button
            size="sm"
            variant="outline"
            onClick={onResetSettings}
            className="flex-1 h-6 text-[10px] gap-1 bg-muted/20 text-muted-foreground hover:text-foreground"
            title="Restore default filters, display, and forces"
          >
            <RotateCcw className="w-3 h-3" />
            <span>Reset Settings</span>
          </Button>

          <Button
            size="sm"
            variant="outline"
            onClick={onResetLayout}
            className="flex-1 h-6 text-[10px] gap-1 bg-muted/20 text-muted-foreground hover:text-destructive"
            title="Clear saved node positions and simulate fresh layout"
          >
            <LayoutGrid className="w-3 h-3" />
            <span>Reset Layout</span>
          </Button>
        </div>
      </div>
    </div>
  );
};
