import { AlertTriangle, ShieldCheck } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { acceptLegalDisclaimer, hasAcceptedLegalDisclaimer } from '@/lib/legal-disclaimer';

export function LegalDisclaimerDialog() {
  const { t } = useTranslation('common');
  const [open, setOpen] = useState(() => !hasAcceptedLegalDisclaimer());

  const handleAccept = () => {
    acceptLegalDisclaimer();
    setOpen(false);
  };

  if (!open) return null;

  return (
    <AlertDialog open={open}>
      <AlertDialogContent className="z-[60] max-w-lg">
        <AlertDialogHeader>
          <div className="mb-2 flex items-center gap-3">
            <div className="rounded-md bg-amber-500/10 p-2 text-amber-600 dark:text-amber-400">
              <AlertTriangle className="h-5 w-5" />
            </div>
            <AlertDialogTitle>{t('legalDisclaimer.title')}</AlertDialogTitle>
          </div>
          <AlertDialogDescription className="space-y-3 text-left leading-relaxed">
            <span className="block">{t('legalDisclaimer.description')}</span>
            <span className="block rounded-md bg-muted/60 p-3 text-xs text-muted-foreground">
              {t('legalDisclaimer.notice')}
            </span>
            <span className="block">{t('legalDisclaimer.responsibility')}</span>
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogAction onClick={handleAccept}>
            <ShieldCheck className="h-4 w-4" />
            {t('legalDisclaimer.accept')}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
