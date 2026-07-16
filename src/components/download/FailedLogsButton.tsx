import { ScrollText } from 'lucide-react';

interface FailedLogsButtonProps {
  label: string;
  onClick: () => void;
}

export function FailedLogsButton({ label, onClick }: FailedLogsButtonProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="inline-flex items-center gap-1 rounded-md border border-dashed border-red-500/35 bg-red-500/5 px-2 py-0.5 text-[11px] font-medium text-red-600 transition-colors hover:border-red-500/60 hover:bg-red-500/10 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-red-500/50 dark:text-red-400"
    >
      <ScrollText className="h-3 w-3" />
      {label}
    </button>
  );
}
