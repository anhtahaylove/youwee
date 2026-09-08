/**
 * Ownership rules shared by the download and universal queues.
 *
 * Both contexts drive a pool of async workers over a single queue. Two bugs are
 * possible without explicit ownership tracking:
 *
 * 1. Double start - a second `startDownload()` in the same tick spawns a second
 *    worker pool with its own claim set, downloading every item twice.
 * 2. Stale teardown - the `finally` block of a run that is still unwinding
 *    clears the shared "is downloading" flags belonging to a newer run that the
 *    user started after Stop, leaving the new run running with a stopped UI.
 *
 * `QueueRunGuard` keeps that logic in one tested place instead of duplicating
 * the ref bookkeeping in each context.
 */
export interface QueueRunHandle {
  /** Identifier of the run that was started, or `null` when start was refused. */
  runId: number | null;
  /** True when this call actually started a run. */
  started: boolean;
}

export class QueueRunGuard {
  private running = false;
  private runId = 0;

  /** True while a run owns the queue. */
  get isRunning(): boolean {
    return this.running;
  }

  /** Id of the run currently owning the queue. */
  get currentRunId(): number {
    return this.runId;
  }

  /**
   * Try to take ownership of the queue. Returns `started: false` when a run is
   * already active, so the caller must return without spawning workers.
   */
  begin(): QueueRunHandle {
    if (this.running) {
      return { runId: null, started: false };
    }
    this.runId += 1;
    this.running = true;
    return { runId: this.runId, started: true };
  }

  /**
   * Whether `runId` still owns the queue and may therefore clear shared state
   * in its `finally` block.
   */
  owns(runId: number): boolean {
    return this.running && this.runId === runId;
  }

  /**
   * Release ownership from a finishing run. A stale run (one that was
   * invalidated by Stop or superseded by a newer run) is ignored.
   */
  finish(runId: number): boolean {
    if (!this.owns(runId)) {
      return false;
    }
    this.running = false;
    return true;
  }

  /**
   * Invalidate the active run, used by Stop. Bumping the id means the stopped
   * run's late `finish()` cannot clear the state of a run started afterwards.
   */
  invalidate(): void {
    this.runId += 1;
    this.running = false;
  }
}
