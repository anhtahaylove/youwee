export const LEGAL_DISCLAIMER_ACCEPTED_KEY = 'youwee_legal_disclaimer_v1_accepted';

export function hasAcceptedLegalDisclaimer(): boolean {
  if (typeof window === 'undefined') return false;
  return window.localStorage.getItem(LEGAL_DISCLAIMER_ACCEPTED_KEY) === 'true';
}

export function acceptLegalDisclaimer() {
  if (typeof window === 'undefined') return;
  window.localStorage.setItem(LEGAL_DISCLAIMER_ACCEPTED_KEY, 'true');
}
