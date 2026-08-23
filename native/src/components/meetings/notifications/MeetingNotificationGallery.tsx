import React, { useState } from 'react';
import { Bell, CheckCircle2, MonitorPlay, Sparkles } from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  MeetingStatus,
  MeetingProvider,
  MeetingNotificationData,
} from './notification-types';
import { MeetingNotificationControls } from './MeetingNotificationControls';
import { MeetingNotificationPreview } from './MeetingNotificationPreview';
import { SimulatedDesktopToastOverlay } from './SimulatedDesktopToastOverlay';
import { NativeInspired } from './variants/NativeInspired';

export const MeetingNotificationGallery: React.FC = () => {
  const [status, setStatus] = useState<MeetingStatus>('upcoming');
  const [provider, setProvider] = useState<MeetingProvider>('Google Meet');
  const [isSimulating, setIsSimulating] = useState<boolean>(false);

  // Simulated state for Native Inspired component
  const [isRecording, setIsRecording] = useState<boolean>(false);
  const [isDismissed, setIsDismissed] = useState<boolean>(false);
  const [snoozedMinutes, setSnoozedMinutes] = useState<number | null>(null);

  const handleToggleRecord = () => {
    setIsRecording((prev) => !prev);
  };

  const handleToggleDismiss = () => {
    setIsDismissed((prev) => !prev);
  };

  const handleSnooze = (mins: number) => {
    setSnoozedMinutes(mins);
  };

  const handleResetAll = () => {
    setIsRecording(false);
    setIsDismissed(false);
    setSnoozedMinutes(null);
    setIsSimulating(false);
  };

  const getTimeLabel = (st: MeetingStatus): string => {
    switch (st) {
      case 'upcoming':
        return 'starts in 5 minutes';
      case 'detected':
        return 'Meeting detected';
      case 'in-progress':
        return 'Meeting in progress';
    }
  };

  const data: MeetingNotificationData = {
    title: 'Design Review',
    status,
    timeLabel: getTimeLabel(status),
    provider,
    participants: 4,
    organizer: 'Nitin • Team',
    scheduledTime: '10:30 AM',
  };

  const nativeNotificationComponent = (
    <NativeInspired
      data={data}
      isRecording={isRecording}
      isDismissed={isDismissed}
      snoozedMinutes={snoozedMinutes}
      actions={{
        onRecord: handleToggleRecord,
        onSnooze: handleSnooze,
        onDismiss: handleToggleDismiss,
      }}
    />
  );

  return (
    <div className="flex-1 flex flex-col space-y-6 select-none max-w-5xl mx-auto w-full pb-16 relative">
      {/* Floating Desktop Simulation Toast Overlay */}
      {isSimulating && (
        <SimulatedDesktopToastOverlay
          variantId="08"
          variantName="Native Inspired"
          onClose={() => setIsSimulating(false)}
        >
          {nativeNotificationComponent}
        </SimulatedDesktopToastOverlay>
      )}

      {/* Header Banner */}
      <div className="relative overflow-hidden rounded-lg border border-border bg-card p-6 shadow-sm">
        <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 relative z-10">
          <div>
            <div className="flex items-center gap-2 mb-2">
              <Badge variant="outline" className="text-xs font-mono border-primary/40 text-primary bg-primary/10">
                Components &gt; Meeting &gt; Notifications
              </Badge>
              <Badge variant="secondary" className="text-xs font-mono gap-1 text-emerald-500 border-emerald-500/30">
                <CheckCircle2 className="w-3 h-3" /> Selected Design (Variant 08)
              </Badge>
            </div>
            <h1 className="text-2xl font-extrabold tracking-tight text-foreground flex items-center gap-2.5">
              <Bell className="w-6 h-6 text-primary" />
              Native Inspired Meeting Notification
            </h1>
            <p className="text-xs text-muted-foreground mt-1 max-w-2xl leading-relaxed">
              Relay's selected visual language for meeting reminders. Features a restrained Windows Toast hierarchy, compact header, clear time &amp; provider metadata, and standard Record, Snooze, and Dismiss action controls.
            </p>
          </div>

          <Button
            type="button"
            size="sm"
            onClick={() => setIsSimulating(true)}
            className="h-9 px-4 text-xs font-bold gap-2 shadow-xs shrink-0"
          >
            <MonitorPlay className="w-4 h-4" /> Simulate Desktop Toast
          </Button>
        </div>
      </div>

      {/* Compare Controls */}
      <MeetingNotificationControls
        status={status}
        setStatus={setStatus}
        provider={provider}
        setProvider={setProvider}
        onResetAll={handleResetAll}
      />

      {/* Selected Variant Showcase Card */}
      <MeetingNotificationPreview
        id="08"
        name="Native Inspired (Selected Production Design)"
        description="Translates cleanly to Windows OS notification toast structures with conservative radius, restrained typography, and standard action row."
        isSelected={true}
        onSelect={() => {}}
        onSimulate={() => setIsSimulating(true)}
      >
        {nativeNotificationComponent}
      </MeetingNotificationPreview>

      {/* Confirmed Selection Banner */}
      <div className="rounded-lg border border-primary/40 bg-primary/5 p-6 shadow-sm space-y-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Sparkles className="w-5 h-5 text-primary" />
            <h2 className="text-base font-bold text-foreground">Selected Notification Design</h2>
          </div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => setIsSimulating(true)}
            className="h-8 text-xs font-semibold gap-1.5 border-primary/40 text-primary hover:bg-primary/10"
          >
            <MonitorPlay className="w-3.5 h-3.5" /> Test Desktop Simulation
          </Button>
        </div>
        <p className="text-xs text-muted-foreground leading-relaxed">
          Variant 08 (Native Inspired) is selected as Relay's baseline visual language for meeting notifications. All other experimental design exploration variants have been scrapped.
        </p>

        <div className="p-3 rounded-md bg-card border border-border text-xs flex items-center justify-between">
          <div className="flex items-center gap-2">
            <CheckCircle2 className="w-4 h-4 text-emerald-500" />
            <span className="font-semibold text-foreground">Active Production Spec:</span>{' '}
            <span className="text-primary font-bold">Variant 08 — Native Inspired</span>
          </div>
          <Badge className="text-[10px] bg-emerald-600 text-white font-mono">
            CONFIRMED
          </Badge>
        </div>
      </div>
    </div>
  );
};
