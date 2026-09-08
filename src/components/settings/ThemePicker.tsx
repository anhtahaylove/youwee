import { Check, Palette } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { useTheme } from '@/contexts/ThemeContext';
import type { ThemeName } from '@/lib/themes';
import { themes } from '@/lib/themes';
import { cn } from '@/lib/utils';

// Gradient backgrounds for theme preview
const themeGradients: Record<ThemeName, string> = {
  midnight: 'bg-gradient-to-br from-indigo-500 via-purple-500 to-pink-500',
  aurora: 'bg-gradient-to-br from-emerald-400 via-cyan-500 to-blue-500',
  sunset: 'bg-gradient-to-br from-orange-500 via-amber-500 to-yellow-500',
  ocean: 'bg-gradient-to-br from-sky-500 via-blue-500 to-indigo-500',
  forest: 'bg-gradient-to-br from-green-500 via-emerald-500 to-teal-500',
  candy: 'bg-gradient-to-br from-pink-500 via-rose-500 to-red-500',
};

export function ThemePicker() {
  const { theme, setTheme } = useTheme();
  // The theme list below binds each item to `t`, so the translator is named
  // `translate` here to avoid shadowing it.
  const { t: translate } = useTranslation('settings');

  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button
          variant="ghost"
          size="icon"
          className="h-9 w-9"
          aria-label={translate('themePicker.changeTheme')}
          title={translate('themePicker.changeTheme')}
        >
          <Palette className="h-4 w-4" aria-hidden="true" />
          <span className="sr-only">{translate('themePicker.changeTheme')}</span>
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-[220px] p-3" align="end">
        <div className="space-y-2">
          <p className="text-sm font-medium text-muted-foreground">
            {translate('themePicker.title')}
          </p>
          <div className="grid grid-cols-3 gap-2">
            {themes.map((t) => (
              <button
                type="button"
                key={t.name}
                aria-pressed={theme === t.name}
                aria-label={t.label}
                onClick={() => setTheme(t.name)}
                className={cn(
                  'flex flex-col items-center gap-1.5 p-2 rounded-lg border-2 transition-all',
                  theme === t.name
                    ? 'border-primary bg-accent'
                    : 'border-transparent hover:bg-accent/50',
                )}
              >
                <div
                  className={cn(
                    'w-8 h-8 rounded-full flex items-center justify-center shadow-lg',
                    themeGradients[t.name],
                  )}
                >
                  {theme === t.name ? (
                    <Check className="w-4 h-4 text-white drop-shadow" aria-hidden="true" />
                  ) : (
                    <span className="text-sm">{t.emoji}</span>
                  )}
                </div>
                <span className="text-xs font-medium">{t.label}</span>
              </button>
            ))}
          </div>
        </div>
      </PopoverContent>
    </Popover>
  );
}
