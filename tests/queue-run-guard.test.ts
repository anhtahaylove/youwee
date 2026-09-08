import { expect, test } from 'bun:test';
import { QueueRunGuard } from '@/lib/queue-run-guard';

test('double start: a second start while a run is active is refused', () => {
  const guard = new QueueRunGuard();

  const first = guard.begin();
  expect(first.started).toBe(true);
  expect(first.runId).toBe(1);

  // Same tick, before any React state update has been committed.
  const second = guard.begin();
  expect(second.started).toBe(false);
  expect(second.runId).toBeNull();

  // Only one run owns the queue, so only one worker pool exists.
  expect(guard.currentRunId).toBe(1);
  expect(guard.isRunning).toBe(true);
});

test('double start: a new run is allowed only after the previous one finishes', () => {
  const guard = new QueueRunGuard();

  const first = guard.begin();
  expect(guard.finish(first.runId as number)).toBe(true);
  expect(guard.isRunning).toBe(false);

  const second = guard.begin();
  expect(second.started).toBe(true);
  expect(second.runId).toBe(2);
});

test('stop/restart: a stale run cannot clear the state of the run started after it', () => {
  const guard = new QueueRunGuard();

  // Run 1 starts, then the user presses Stop while its workers are still
  // unwinding in the background.
  const stale = guard.begin();
  guard.invalidate();
  expect(guard.isRunning).toBe(false);

  // The user immediately starts a new run.
  const fresh = guard.begin();
  expect(fresh.started).toBe(true);
  expect(fresh.runId).not.toBe(stale.runId);

  // The stale run's `finally` block finally runs. It must not clear the flags
  // now owned by the fresh run.
  expect(guard.finish(stale.runId as number)).toBe(false);
  expect(guard.isRunning).toBe(true);
  expect(guard.owns(fresh.runId as number)).toBe(true);

  // The fresh run may still clean up after itself.
  expect(guard.finish(fresh.runId as number)).toBe(true);
  expect(guard.isRunning).toBe(false);
});

test('stop/restart: stop does not wedge the guard against later starts', () => {
  const guard = new QueueRunGuard();

  guard.begin();
  guard.invalidate();

  // Stop must leave the guard startable, otherwise the queue is dead until reload.
  expect(guard.begin().started).toBe(true);
});

test('stop/restart: stopping with no active run stays consistent', () => {
  const guard = new QueueRunGuard();

  guard.invalidate();
  expect(guard.isRunning).toBe(false);

  const run = guard.begin();
  expect(run.started).toBe(true);
  expect(guard.owns(run.runId as number)).toBe(true);
});

test('repeated stop/restart cycles never leave two runs owning the queue', () => {
  const guard = new QueueRunGuard();
  const owning: number[] = [];

  for (let i = 0; i < 25; i += 1) {
    const run = guard.begin();
    if (run.started) {
      owning.push(run.runId as number);
    }
    // Interleave a stop half the time, leaving the previous run to unwind late.
    if (i % 2 === 0) {
      guard.invalidate();
    }
  }

  // Every stale run reports in afterwards, in arbitrary order.
  const accepted = owning.filter((runId) => guard.finish(runId));

  // At most one run can ever be the owner that clears shared state.
  expect(accepted.length).toBeLessThanOrEqual(1);
});
