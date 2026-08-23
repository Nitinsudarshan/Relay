export type MeetingStatus = 'upcoming' | 'detected' | 'in-progress';

export type MeetingProvider =
  | 'Google Meet'
  | 'Zoom'
  | 'Teams'
  | 'Webex'
  | 'In Person';

export interface MeetingNotificationData {
  title: string;
  status: MeetingStatus;
  timeLabel: string;
  provider: MeetingProvider;
  participants?: number;
  organizer?: string;
  scheduledTime?: string;
}

export interface NotificationActions {
  onRecord?: () => void;
  onSnooze?: (minutes: number) => void;
  onDismiss?: () => void;
}

export interface VariantProps {
  data: MeetingNotificationData;
  actions: NotificationActions;
  isRecording?: boolean;
  isDismissed?: boolean;
  snoozedMinutes?: number | null;
}
