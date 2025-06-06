use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::hash::BuildHasher;
use std::num::NonZeroU64;

use serde::Serialize;
use tokio::time::Instant;

use super::job::Job;
use super::states::JobState;

// Required tube functionality:
// * State transitions:
//   * Reserve by ID or by head of tube.
//   * Release by ID.
//   * Bury/unbury by ID.
//   * Touch by ID.
//   * Delayed -> Ready.
// * Meta:
//   * Count jobs in the tube by state.
//   * Get job stats or data by ID.
// NB: reserve by ID, delete are global operations that can be performed
// regardless of the queue being watched by the client.
// NB: bury and touch can be executed regardless of the current watch set,
// provided the client reserved that particular job.

#[derive(Debug, PartialEq, Serialize)]
pub struct TubeStats {
    /// number of jobs in ready state with priority < 1024
    #[serde(rename = "current-jobs-urgent")]
    pub current_jobs_urgent: u64,
    /// number of jobs in ready state
    #[serde(rename = "current-jobs-ready")]
    pub current_jobs_ready: u64,
    /// number of jobs reserved by clients
    #[serde(rename = "current-jobs-reserved")]
    pub current_jobs_reserved: u64,
    /// number of jobs in delayed state
    #[serde(rename = "current-jobs-delayed")]
    pub current_jobs_delayed: u64,
    /// number of jobs in buried state
    #[serde(rename = "current-jobs-buried")]
    pub current_jobs_buried: u64,
    /// total jobs created in this tube
    #[serde(rename = "total-jobs")]
    pub total_jobs: u64,
    /// number of clients that have `use`d this queue
    #[serde(rename = "current-using")]
    pub current_using: u64,
    /// number of clients that have `watch`ed this queue and are waiting on a
    /// `reserve`
    #[serde(rename = "current-waiting")]
    pub current_waiting: u64,
    /// number of clients that have `watch`ed this queue
    #[serde(rename = "current-watching")]
    pub current_watching: u64,
    /// number of seconds this queue has been paused for in total
    pub pause: u32,
    /// number of `delete` commands issued for this tube
    #[serde(rename = "cmd-delete")]
    pub cmd_delete: u64,
    /// number of `pause-tube` commands issued for this tube
    #[serde(rename = "cmd-pause-tube")]
    pub cmd_pause_tube: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
struct JobId(NonZeroU64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct BuriedPos(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct ReadyPos(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct Pri(u32);

#[derive(Debug, Eq, PartialEq, PartialOrd, Ord)]
struct QueueName(Vec<u8>);

#[derive(Debug)]
struct QueueSet(HashSet<QueueName>);

pub struct TubeState {
    buried: BTreeMap<BuriedPos, JobId>, // position -> job ID
    buried_sn: BuriedPos,
    ready: BTreeMap<ReadyPos, JobId>, // position -> job ID
    ready_sn: ReadyPos,
    // NB: Instants are only non-decreasing, so must tolerate duplication.
    delayed: BTreeSet<(Instant, JobId)>, // (ready time, job ID)
    pause_until: Option<Instant>,
    stats: TubeStats,
}

// TODO: make it configurable if jobs that time out and re-enter the queue go to
// the back or front. Defaulting to the back may be sensible.
// TODO: allow the user to configure a limit to ensure a job that times out N
// times gets buried.
impl TubeState {
    /// Inserts a job into the ready queue. Panics if the job ID is already
    /// present.
    fn put_ready(&mut self, job_id: JobId) -> ReadyPos {
        let rp = self.ready_sn;
        self.ready_sn = ReadyPos(self.ready_sn.0 + 1);

        assert!(self.ready.insert(rp, job_id).is_none());

        self.stats.current_jobs_ready += 1;

        rp
    }

    /// Inserts a job into the buried queue.
    fn put_buried(&mut self, job_id: JobId) -> BuriedPos {
        let bp = self.buried_sn;
        self.buried_sn = BuriedPos(self.buried_sn.0 + 1);

        assert!(self.buried.insert(bp, job_id).is_none());

        self.stats.current_jobs_buried += 1;

        bp
    }

    /// Inserts a job into the delayed queue.
    fn put_delayed(&mut self, job_id: JobId, until: Instant) {
        assert!(self.delayed.insert((until, job_id)));

        self.stats.current_jobs_delayed += 1;
    }

    /// Removes a job from the buried list.
    fn take_buried(&mut self, pos: &BuriedPos) {
        self.buried.remove(pos).unwrap();

        self.stats.current_jobs_buried -= 1;
    }

    /// Mark a job at a given position as reserved, removing it from the ready
    /// queue. Panics if that job doesn't exist in the ready queue.
    fn take_ready(&mut self, pos: ReadyPos) {
        self.ready.remove(&pos).unwrap();

        self.stats.current_jobs_ready -= 1;
    }
}

pub struct Server {
    id: &'static str,
    jobs: BTreeMap<JobId, (QueueName, Job)>,
    queues: BTreeMap<QueueName, TubeState>,
    is_draining: bool,
}

impl Server {
    /// Reserves a job by ID, returning its contents.
    fn reserve_by_id(&mut self, id: JobId) -> Option<&Job> {
        let (qn, job) = self.jobs.get(&id)?;

        let JobState::Ready { pos } = job.state else {
            return None;
        };

        // Panic safety: a queue must exist if any jobs reference it, so this
        // should be safe if correctly implemented.
        let queue = self.queues.get_mut(qn).unwrap();

        queue.take_ready(pos);

        Some(job)
    }

    /// Releases a job by ID, returning a boolean indicating success.
    fn release(&mut self, id: JobId) -> bool {
        let Some((qn, job)) = self.jobs.get(&id) else {
            return false;
        };

        todo!()
    }

    /// Reserves the highest-priority ready job from the provided QueueSet,
    /// ignoring paused queues.
    ///
    /// Stochastic fairness is supported: when scanning for the highest-priority
    /// job in the queueset, the provided [BuildHasher] is used to randomise
    /// which queue wins.
    fn reserve_by_queue(
        &mut self,
        qs: QueueSet,
        h: impl BuildHasher,
    ) -> Option<&Job> {
        todo!()
    }

    /// Buries a job by ID, returning a boolean indicating if this occurred.
    fn bury(&mut self, id: JobId) -> bool {
        todo!()
    }

    /// Kicks a job by ID, returning a boolean indicating if this occurred.
    fn kick(&mut self, id: JobId) -> bool {
        todo!()
    }

    /// Refreshes a job's TTR, returning a boolean indicating success.
    fn touch(&mut self, id: JobId) -> bool {
        todo!()
    }

    /// Pushes any delayed jobs that have become ready into the ready queue.
    fn handle_delayed_jobs(&mut self) {
        todo!()
    }
}
