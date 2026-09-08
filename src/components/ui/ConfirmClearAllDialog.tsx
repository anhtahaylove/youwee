import { useTranslation } from 'react-i18next';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { buttonVariants } from '@/components/ui/button';
import { cn } from '@/lib/utils';

interface ConfirmClearAllDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Number of queue items that will be discarded. */
  itemCount: number;
  onConfirm: () => void;
}

/**
 * Confirmation step for "clear all" queue actions.
 *
 * Clearing a queue is destructive and cannot be undone, so every caller routes
 * through this dialog instead of wiring the action straight to the button.
 */
export function ConfirmClearAllDialog({
  open,
  onOpenChange,
  itemCount,
  onConfirm,
}: ConfirmClearAllDialogProps) {
  const { t } = useTranslation('common');

  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{t('clearAllDialog.title')}</AlertDialogTitle>
          <AlertDialogDescription>
            {t('clearAllDialog.description', { count: itemCount })}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>{t('actions.cancel')}</AlertDialogCancel>
          <AlertDialogAction
            className={cn(buttonVariants({ variant: 'destructive' }))}
            onClick={onConfirm}
          >
            {t('clearAllDialog.confirm')}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
