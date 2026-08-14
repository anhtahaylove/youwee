import { Info } from 'lucide-react';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import type { VideoCompatibilityMode } from '@/lib/types';

interface VideoCompatibilityPresetProps {
  value: VideoCompatibilityMode;
  disabled?: boolean;
  label: string;
  originalLabel: string;
  originalDescription: string;
  h264Label: string;
  h264Description: string;
  onValueChange: (mode: VideoCompatibilityMode) => void;
}

export function VideoCompatibilityPreset({
  value,
  disabled,
  label,
  originalLabel,
  originalDescription,
  h264Label,
  h264Description,
  onValueChange,
}: VideoCompatibilityPresetProps) {
  const description = value === 'h264' ? h264Description : originalDescription;

  return (
    <div className="flex items-center gap-1">
      <Select value={value} onValueChange={onValueChange} disabled={disabled}>
        <SelectTrigger
          className="h-9 w-[152px] bg-card/50 text-xs border-border/50"
          aria-label={label}
          title={`${label}: ${description}`}
        >
          <SelectValue>{value === 'h264' ? h264Label : originalLabel}</SelectValue>
        </SelectTrigger>
        <SelectContent className="min-w-[190px]">
          <SelectItem value="original" className="text-xs">
            {originalLabel}
          </SelectItem>
          <SelectItem value="h264" className="text-xs">
            {h264Label}
          </SelectItem>
        </SelectContent>
      </Select>
      <TooltipProvider>
        <Tooltip>
          <TooltipTrigger asChild>
            <button
              type="button"
              className="inline-flex h-8 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
              aria-label={`${label}: ${description}`}
              disabled={disabled}
            >
              <Info className="h-3.5 w-3.5" />
            </button>
          </TooltipTrigger>
          <TooltipContent className="max-w-72 text-xs">{description}</TooltipContent>
        </Tooltip>
      </TooltipProvider>
    </div>
  );
}
