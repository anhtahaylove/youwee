import { expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import { FailedLogsButton } from '@/components/download/FailedLogsButton';

test('failed log hint renders as an actionable button', () => {
  const html = renderToStaticMarkup(
    <FailedLogsButton label="View Logs page for details" onClick={() => {}} />,
  );

  expect(html).toContain('<button');
  expect(html).toContain('View Logs page for details');
});
