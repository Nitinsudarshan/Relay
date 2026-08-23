import React from 'react';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Button } from '@/components/ui/button';
import { Clock } from 'lucide-react';

interface SnoozeMenuProps {
  onSnooze?: (minutes: number) => void;
  variant?: 'default' | 'outline' | 'ghost' | 'secondary';
  size?: 'default' | 'sm' | 'lg' | 'icon';
  className?: string;
  label?: string;
}

export const SnoozeMenu: React.FC<SnoozeMenuProps> = ({
  onSnooze,
  variant = 'outline',
  size = 'sm',
  className = '',
  label = 'Snooze',
}) => {
  const options = [5, 10, 15, 30];

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          type="button"
          variant={variant}
          size={size}
          className={`gap-1 font-medium ${className}`}
        >
          <Clock className="w-3 h-3 text-muted-foreground" />
          <span>{label}</span>
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-36 text-xs">
        <div className="px-2 py-1 text-[10px] font-semibold text-muted-foreground uppercase tracking-wider">
          Snooze duration
        </div>
        {options.map((mins) => (
          <DropdownMenuItem
            key={mins}
            onClick={() => onSnooze?.(mins)}
            className="cursor-pointer text-xs flex items-center justify-between"
          >
            <span>{mins} minutes</span>
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
};
