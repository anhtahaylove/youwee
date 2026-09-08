import { expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import { BrowserCookieErrorDialog } from '@/components/BrowserCookieErrorDialog';
import { FreshCookieRequiredDialog } from '@/components/FreshCookieRequiredDialog';
import { ModalShell } from '@/components/ui/ModalShell';

test('modal shell exposes dialog semantics and an accessible name', () => {
  const html = renderToStaticMarkup(
    <ModalShell onDismiss={() => {}} titleId="t1" descriptionId="d1">
      <h2 id="t1">Title</h2>
      <p id="d1">Description</p>
    </ModalShell>,
  );

  expect(html).toContain('role="dialog"');
  expect(html).toContain('aria-modal="true"');
  expect(html).toContain('aria-labelledby="t1"');
  expect(html).toContain('aria-describedby="d1"');
});

test('hand-built dialogs are labelled by their visible heading', () => {
  const html = renderToStaticMarkup(<FreshCookieRequiredDialog onDismiss={() => {}} />);

  // The aria-labelledby target must actually exist in the markup, otherwise the
  // dialog announces with no name at all.
  const labelledBy = html.match(/aria-labelledby="([^"]+)"/);
  expect(labelledBy).not.toBeNull();
  expect(html).toContain(`id="${labelledBy?.[1]}"`);
  expect(html).toContain('Login Cookies Required');
});

test('icon-only close control carries a text label', () => {
  const html = renderToStaticMarkup(<FreshCookieRequiredDialog onDismiss={() => {}} />);

  expect(html).toContain('aria-label="Close"');
});

test('dialogs no longer render a bare unlabelled overlay div', () => {
  const html = renderToStaticMarkup(
    <BrowserCookieErrorDialog onRetry={() => {}} onDismiss={() => {}} />,
  );

  // Guard against a regression back to the plain <div> overlay with no role.
  expect(html).toContain('role="dialog"');
});
