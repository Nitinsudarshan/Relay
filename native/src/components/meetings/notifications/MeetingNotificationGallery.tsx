import React, { useState } from 'react';
import { Bell, Check, Sparkles, MonitorPlay } from 'lucide-react';
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

import { ClassicCompact } from './variants/ClassicCompact';
import { Executive } from './variants/Executive';
import { FloatingCard } from './variants/FloatingCard';
import { StatusFirst } from './variants/StatusFirst';
import { ActionFirst } from './variants/ActionFirst';
import { Minimal } from './variants/Minimal';
import { RichContext } from './variants/RichContext';
import { NativeInspired } from './variants/NativeInspired';

export const MeetingNotificationGallery: React.FC = () => {
  const [status, setStatus] = useState<MeetingStatus>('upcoming');
  const [provider, setProvider] = useState<MeetingProvider>('Google Meet');
  const [selectedDirection, setSelectedDirection] = useState<string>('01');
  const [activeSimulationId, setActiveSimulationId] = useState<string | null>(null);

  // Simulated state per variant id (1..8)
  const [recordingStates, setRecordingStates] = useState<Record<string, boolean>>({});
  const [dismissedStates, setDismissedStates] = useState<Record<string, boolean>>({});
  const [snoozeStates, setSnoozeStates] = useState<Record<string, number | null>>({});

  const handleToggleRecord = (id: string) => {
    setRecordingStates((prev) => ({ ...prev, [id]: !prev[id] }));
  };

  const handleToggleDismiss = (id: string) => {
    setDismissedStates((prev) => ({ ...prev, [id]: !prev[id] }));
  };

  const handleSnooze = (id: string, mins: number) => {
    setSnoozeStates((prev) => ({ ...prev, [id]: mins }));
  };

  const handleResetAll = () => {
    setRecordingStates({});
    setDismissedStates({});
    setSnoozeStates({});
    setActiveSimulationId(null);
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

  const renderVariantComponent = (id: string) => {
    switch (id) {
      case '01':
        return (
          <ClassicCompact
            data={data}
            isRecording={recordingStates['01']}
            isDismissed={dismissedStates['01']}
            snoozedMinutes={snoozeStates['01']}
            actions={{
              onRecord: () => handleToggleRecord('01'),
              onSnooze: (mins) => handleSnooze('01', mins),
              onDismiss: () => handleToggleDismiss('01'),
            }}
          />
        );
      case '02':
        return (
          <Executive
            data={data}
            isRecording={recordingStates['02']}
            isDismissed={dismissedStates['02']}
            snoozedMinutes={snoozeStates['02']}
            actions={{
              onRecord: () => handleToggleRecord('02'),
              onSnooze: (mins) => handleSnooze('02', mins),
              onDismiss: () => handleToggleDismiss('02'),
            }}
          />
        );
      case '03':
        return (
          <FloatingCard
            data={data}
            isRecording={recordingStates['03']}
            isDismissed={dismissedStates['03']}
            snoozedMinutes={snoozeStates['03']}
            actions={{
              onRecord: () => handleToggleRecord('03'),
              onSnooze: (mins) => handleSnooze('03', mins),
              onDismiss: () => handleToggleDismiss('03'),
            }}
          />
        );
      case '04':
        return (
          <StatusFirst
            data={data}
            isRecording={recordingStates['04']}
            isDismissed={dismissedStates['04']}
            snoozedMinutes={snoozeStates['04']}
            actions={{
              onRecord: () => handleToggleRecord('04'),
              onSnooze: (mins) => handleSnooze('04', mins),
              onDismiss: () => handleToggleDismiss('04'),
            }}
          />
        );
      case '05':
        return (
          <ActionFirst
            data={data}
            isRecording={recordingStates['05']}
            isDismissed={dismissedStates['05']}
            snoozedMinutes={snoozeStates['05']}
            actions={{
              onRecord: () => handleToggleRecord('05'),
              onSnooze: (mins) => handleSnooze('05', mins),
              onDismiss: () => handleToggleDismiss('05'),
            }}
          />
        );
      case '06':
        return (
          <Minimal
            data={data}
            isRecording={recordingStates['06']}
            isDismissed={dismissedStates['06']}
            snoozedMinutes={snoozeStates['06']}
            actions={{
              onRecord: () => handleToggleRecord('06'),
              onSnooze: (mins) => handleSnooze('06', mins),
              onDismiss: () => handleToggleDismiss('06'),
            }}
          />
        );
      case '07':
        return (
          <RichContext
            data={data}
            isRecording={recordingStates['07']}
            isDismissed={dismissedStates['07']}
            snoozedMinutes={snoozeStates['07']}
            actions={{
              onRecord: () => handleToggleRecord('07'),
              onSnooze: (mins) => handleSnooze('07', mins),
              onDismiss: () => handleToggleDismiss('07'),
            }}
          />
        );
      case '08':
        return (
          <NativeInspired
            data={data}
            isRecording={recordingStates['08']}
            isDismissed={dismissedStates['08']}
            snoozedMinutes={snoozeStates['08']}
            actions={{
              onRecord: () => handleToggleRecord('08'),
              onSnooze: (mins) => handleSnooze('08', mins),
              onDismiss: () => handleToggleDismiss('08'),
            }}
          />
        );
      default:
        return null;
    }
  };

  const variants = [
    {
      id: '01',
      name: 'Classic Compact',
      description:
        'Restrained baseline interpretation of reference notification with strong hierarchy and minimal decoration.',
    },
    {
      id: '02',
      name: 'Executive',
      description:
        'Polished executive productivity aesthetic with strong title hierarchy and subtle metadata.',
    },
    {
      id: '03',
      name: 'Floating Card',
      description:
        'Softer floating-card aesthetic with rounded corners, generous padding, and provider badge.',
    },
    {
      id: '04',
      name: 'Status First',
      description:
        'Dominant status badge and state indicator for immediate visual urgency at a glance.',
    },
    {
      id: '05',
      name: 'Action First',
      description:
        'Extremely compact metadata prioritizing large Record, Snooze, and Dismiss buttons for interaction speed.',
    },
    {
      id: '06',
      name: 'Minimal',
      description:
        'Aggressively stripped down with minimal text, minimal chrome, and high information density.',
    },
    {
      id: '07',
      name: 'Rich Context',
      description:
        'Displays attendee counts, organizer info, and calendar context without visual clutter.',
    },
    {
      id: '08',
      name: 'Native Inspired',
      description:
        'Translates directly to native OS notification toast structures with simple buttons and conservative radius.',
    },
  ];

  const selectedVariantObj = variants.find((v) => v.id === selectedDirection) || variants[0];
  const activeSimulatedVariantObj = variants.find((v) => v.id === activeSimulationId);

  return (
    <div className="flex-1 flex flex-col space-y-6 select-none max-w-7xl mx-auto w-full pb-16 relative">
      {/* Floating Desktop Simulation Toast Overlay */}
      {activeSimulationId && activeSimulatedVariantObj && (
        <SimulatedDesktopToastOverlay
          variantId={activeSimulatedVariantObj.id}
          variantName={activeSimulatedVariantObj.name}
          onClose={() => setActiveSimulationId(null)}
        >
          {renderVariantComponent(activeSimulationId)}
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
              <Badge variant="secondary" className="text-xs font-mono">
                Interactive Prototyping Surface
              </Badge>
            </div>
            <h1 className="text-2xl font-extrabold tracking-tight text-foreground flex items-center gap-2.5">
              <Bell className="w-6 h-6 text-primary" />
              Meeting Notifications
            </h1>
            <p className="text-xs text-muted-foreground mt-1 max-w-2xl leading-relaxed">
              Explore notification treatments for Relay's meeting reminders. Compare 8 visual interpretations side-by-side or trigger a live top-right desktop toast simulation.
            </p>
          </div>

          <Button
            type="button"
            size="sm"
            onClick={() => setActiveSimulationId(selectedDirection)}
            className="h-9 px-4 text-xs font-bold gap-2 shadow-xs shrink-0"
          >
            <MonitorPlay className="w-4 h-4" /> Simulate Variant {selectedDirection}
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

      {/* Variant Grid (2 columns on medium screens+) */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        {variants.map((variant) => (
          <MeetingNotificationPreview
            key={variant.id}
            id={variant.id}
            name={variant.name}
            description={variant.description}
            isSelected={selectedDirection === variant.id}
            onSelect={() => setSelectedDirection(variant.id)}
            onSimulate={() => setActiveSimulationId(variant.id)}
          >
            {renderVariantComponent(variant.id)}
          </MeetingNotificationPreview>
        ))}
      </div>

      {/* Recommendation & Direction Selection Section */}
      <div className="rounded-lg border border-border bg-card p-6 shadow-sm space-y-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Sparkles className="w-5 h-5 text-primary" />
            <h2 className="text-lg font-bold text-foreground">Choose a direction</h2>
          </div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => setActiveSimulationId(selectedDirection)}
            className="h-8 text-xs font-semibold gap-1.5"
          >
            <MonitorPlay className="w-3.5 h-3.5 text-primary" /> Simulate Active Direction
          </Button>
        </div>
        <p className="text-xs text-muted-foreground leading-relaxed">
          Select your preferred notification treatment to help guide Relay's visual language when translating interactions to native desktop surfaces.
        </p>

        <div className="flex flex-wrap items-center gap-2 pt-2">
          {variants.map((v) => (
            <Button
              key={v.id}
              type="button"
              variant={selectedDirection === v.id ? 'default' : 'outline'}
              size="sm"
              onClick={() => setSelectedDirection(v.id)}
              className="h-8 text-xs gap-1.5 font-medium"
            >
              {selectedDirection === v.id && <Check className="w-3.5 h-3.5" />}
              <span>
                {v.id} — {v.name}
              </span>
            </Button>
          ))}
        </div>

        <div className="p-4 rounded-md bg-muted/40 border border-border/60 text-xs flex items-center justify-between">
          <div>
            <span className="font-semibold text-foreground">Active Selection:</span>{' '}
            <span className="text-primary font-bold">
              Variant {selectedVariantObj.id} — {selectedVariantObj.name}
            </span>
          </div>
          <Badge variant="outline" className="text-xs border-primary/30 text-primary">
            Selected Direction
          </Badge>
        </div>
      </div>
    </div>
  );
};
