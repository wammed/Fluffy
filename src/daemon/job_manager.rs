use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::SystemTime,
};

use crate::ipc::{
    protocol::{ConversionJobInfo, JobState},
    server::ClientId,
};

pub type JobId = u64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobSubscriber {
    pub request_id: u64,
    pub client_id: Option<ClientId>,
    pub generation: u64,
    pub target_output: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConversionJob {
    pub id: JobId,
    pub source: PathBuf,
    pub content_hash: Option<String>,
    pub state: JobState,
    pub started_at: SystemTime,
    pub error: Option<String>,
    pub cancel_token: Arc<AtomicBool>,
    /// Associated client requests waiting for this job to complete.
    /// Each subscriber tracks its own target output, request generation, and IPC client ID.
    pub subscribers: Vec<JobSubscriber>,
}

impl ConversionJob {
    pub fn is_in_flight(&self) -> bool {
        matches!(
            self.state,
            JobState::Queued | JobState::Probing | JobState::Transcoding | JobState::Installing
        )
    }

    /// Returns the maximum generation among all attached subscribers.
    pub fn latest_generation(&self) -> u64 {
        self.subscribers
            .iter()
            .map(|s| s.generation)
            .max()
            .unwrap_or(0)
    }

    /// Summarizes target outputs across all subscribers.
    pub fn target_outputs_summary(&self) -> Option<String> {
        let mut outputs: Vec<&str> = self
            .subscribers
            .iter()
            .filter_map(|s| s.target_output.as_deref())
            .collect();
        outputs.sort_unstable();
        outputs.dedup();

        if outputs.is_empty() {
            None
        } else if outputs.len() == 1 {
            Some(outputs[0].to_string())
        } else {
            Some(outputs.join(", "))
        }
    }

    pub fn to_info(&self) -> ConversionJobInfo {
        ConversionJobInfo {
            job_id: self.id,
            source: self.source.to_string_lossy().to_string(),
            generation: self.latest_generation(),
            target_output: self.target_outputs_summary(),
            state: self.state,
            error: self.error.clone(),
        }
    }
}

pub struct JobManager {
    jobs: HashMap<JobId, ConversionJob>,
    next_job_id: u64,
}

impl Default for JobManager {
    fn default() -> Self {
        Self::new()
    }
}

impl JobManager {
    pub fn new() -> Self {
        Self {
            jobs: HashMap::new(),
            next_job_id: 1,
        }
    }

    /// Registers a conversion job.
    ///
    /// If an in-flight job for the same content_hash (or exact same source path) already exists,
    /// the caller is added as a subscriber to that job rather than spawning a duplicate worker.
    ///
    /// Each subscriber maintains its own `request_id`, `client_id`, `generation`, and `target_output`,
    /// ensuring that multi-output requests to the same video share a single transcode while correctly
    /// applying to their respective outputs upon completion.
    ///
    /// Returns `(job_id, is_new)` where `is_new == true` indicates a new worker thread should be spawned.
    pub fn register_job(
        &mut self,
        source: &Path,
        content_hash: Option<&str>,
        generation: u64,
        target_output: Option<String>,
        request_id: u64,
        client_id: Option<ClientId>,
    ) -> (JobId, bool) {
        let subscriber = JobSubscriber {
            request_id,
            client_id,
            generation,
            target_output,
        };

        // Check for an existing in-flight job with matching content hash or source
        for job in self.jobs.values_mut() {
            let hash_match = content_hash.is_some() && job.content_hash.as_deref() == content_hash;
            let source_match = job.source == source;

            if job.is_in_flight() && (hash_match || source_match) {
                tracing::info!(
                    "[JobManager] In-Flight deduplication hit (hash: {:?}, source: {:?}): attaching request #{} (output: {:?}, gen: {}) to existing Job #{}",
                    content_hash,
                    source,
                    subscriber.request_id,
                    subscriber.target_output,
                    subscriber.generation,
                    job.id
                );
                if job.content_hash.is_none() && content_hash.is_some() {
                    job.content_hash = content_hash.map(|s| s.to_string());
                }
                job.subscribers.push(subscriber);
                return (job.id, false);
            }
        }

        let job_id = self.next_job_id;
        self.next_job_id = self.next_job_id.saturating_add(1);

        let job = ConversionJob {
            id: job_id,
            source: source.to_path_buf(),
            content_hash: content_hash.map(|s| s.to_string()),
            state: JobState::Queued,
            started_at: SystemTime::now(),
            error: None,
            cancel_token: Arc::new(AtomicBool::new(false)),
            subscribers: vec![subscriber],
        };

        self.jobs.insert(job_id, job);
        (job_id, true)
    }

    pub fn find_in_flight_by_hash(&self, hash: &str) -> Option<&ConversionJob> {
        self.jobs
            .values()
            .find(|j| j.is_in_flight() && j.content_hash.as_deref() == Some(hash))
    }

    pub fn get_job(&self, id: JobId) -> Option<&ConversionJob> {
        self.jobs.get(&id)
    }

    pub fn get_job_mut(&mut self, id: JobId) -> Option<&mut ConversionJob> {
        self.jobs.get_mut(&id)
    }

    pub fn cancel_token(&self, id: JobId) -> Option<Arc<AtomicBool>> {
        self.jobs.get(&id).map(|j| j.cancel_token.clone())
    }

    pub fn set_state(&mut self, id: JobId, state: JobState) {
        if let Some(job) = self.jobs.get_mut(&id) {
            job.state = state;
        }
    }

    pub fn complete_job(&mut self, id: JobId) {
        if let Some(job) = self.jobs.get_mut(&id) {
            job.state = JobState::Completed;
        }
        self.prune_finished_jobs(50);
    }

    pub fn fail_job(&mut self, id: JobId, error: String) {
        if let Some(job) = self.jobs.get_mut(&id) {
            job.state = JobState::Failed;
            job.error = Some(error);
        }
        self.prune_finished_jobs(50);
    }

    pub fn cancel_job(&mut self, id: JobId) {
        if let Some(job) = self.jobs.get_mut(&id) {
            job.cancel_token.store(true, Ordering::SeqCst);
            job.state = JobState::Cancelled;
        }
        self.prune_finished_jobs(50);
    }

    /// Cancels in-flight jobs targeting the given output whose generation is superseded by `new_generation`,
    /// excluding an optional currently registered job ID (e.g. newly deduplicated in-flight job).
    /// Immediately signals the cancellation token so any executing ffmpeg processes terminate.
    pub fn cancel_superseded_jobs_except(
        &mut self,
        target_output: Option<&str>,
        new_generation: u64,
        exclude_job_id: Option<JobId>,
    ) -> Vec<JobId> {
        let mut superseded_ids = Vec::new();

        for job in self.jobs.values() {
            if !job.is_in_flight() {
                continue;
            }

            if Some(job.id) == exclude_job_id {
                continue;
            }

            let matches_output = match target_output {
                None => true, // global switch supersedes all older in-flight jobs
                Some(tgt) => {
                    // Output-specific switch: check if this job only targets the same output
                    job.subscribers
                        .iter()
                        .all(|s| s.target_output.as_deref() == Some(tgt))
                }
            };

            if matches_output && job.latest_generation() < new_generation {
                superseded_ids.push(job.id);
            }
        }

        for &id in &superseded_ids {
            tracing::info!(
                operation = "job_cancel_superseded",
                job_id = id,
                new_generation,
                "[JobManager] Superseded in-flight conversion job cancelled"
            );
            self.cancel_job(id);
        }

        superseded_ids
    }

    /// Cancels in-flight jobs targeting the given output whose generation is superseded by `new_generation`.
    /// Immediately signals the cancellation token so any executing ffmpeg processes terminate.
    pub fn cancel_superseded_jobs(
        &mut self,
        target_output: Option<&str>,
        new_generation: u64,
    ) -> Vec<JobId> {
        self.cancel_superseded_jobs_except(target_output, new_generation, None)
    }

    pub fn mark_stale(&mut self, id: JobId) {
        if let Some(job) = self.jobs.get_mut(&id) {
            job.state = JobState::Stale;
        }
        self.prune_finished_jobs(50);
    }

    /// Prunes finished (non-in-flight) jobs keeping at most `keep_max` recent entries.
    pub fn prune_finished_jobs(&mut self, keep_max: usize) {
        let mut finished_ids: Vec<JobId> = self
            .jobs
            .values()
            .filter(|j| !j.is_in_flight())
            .map(|j| j.id)
            .collect();
        finished_ids.sort();

        if finished_ids.len() > keep_max {
            let to_remove = finished_ids.len() - keep_max;
            for id in finished_ids.into_iter().take(to_remove) {
                self.jobs.remove(&id);
            }
        }
    }

    /// Returns a list of currently in-flight conversion job statuses, sorted deterministically by job_id.
    pub fn active_jobs(&self) -> Vec<ConversionJobInfo> {
        let mut list: Vec<ConversionJobInfo> = self
            .jobs
            .values()
            .filter(|j| j.is_in_flight())
            .map(|j| j.to_info())
            .collect();
        list.sort_by_key(|j| j.job_id);
        list
    }

    pub fn is_converting(&self) -> bool {
        self.jobs.values().any(|j| j.is_in_flight())
    }

    pub fn active_job_count(&self) -> usize {
        self.jobs.values().filter(|j| j.is_in_flight()).count()
    }

    pub fn current_converting_file(&self) -> Option<String> {
        self.jobs
            .values()
            .filter(|j| j.is_in_flight())
            .last()
            .map(|j| {
                j.source
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| j.source.display().to_string())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_manager_three_jobs_concurrently() {
        let mut mgr = JobManager::new();

        let (id1, new1) = mgr.register_job(
            Path::new("videoA.mp4"),
            None,
            10,
            Some("DP-1".into()),
            101,
            None,
        );
        let (id2, new2) = mgr.register_job(
            Path::new("videoB.mkv"),
            None,
            11,
            Some("DP-1".into()),
            102,
            None,
        );
        let (id3, new3) = mgr.register_job(Path::new("videoC.webm"), None, 12, None, 103, None);

        assert!(new1 && new2 && new3);
        assert_eq!((id1, id2, id3), (1, 2, 3));

        assert!(mgr.is_converting());
        assert_eq!(mgr.active_job_count(), 3);

        let active = mgr.active_jobs();
        assert_eq!(active.len(), 3);
        assert_eq!(active[0].job_id, 1);
        assert_eq!(active[1].job_id, 2);
        assert_eq!(active[2].job_id, 3);
    }

    #[test]
    fn test_job_manager_one_finished_two_continue() {
        let mut mgr = JobManager::new();

        let (id1, _) = mgr.register_job(Path::new("videoA.mp4"), None, 10, None, 101, None);
        let (id2, _) = mgr.register_job(Path::new("videoB.mp4"), None, 11, None, 102, None);
        let (id3, _) = mgr.register_job(Path::new("videoC.mp4"), None, 12, None, 103, None);

        assert_eq!(mgr.active_job_count(), 3);

        // Job 1 completes
        mgr.complete_job(id1);

        assert!(
            mgr.is_converting(),
            "Daemon must still report is_converting=true while other jobs run"
        );
        assert_eq!(mgr.active_job_count(), 2);

        let active = mgr.active_jobs();
        assert_eq!(active.len(), 2);
        assert_eq!(active[0].job_id, id2);
        assert_eq!(active[1].job_id, id3);

        // Job 2 and 3 complete
        mgr.complete_job(id2);
        assert_eq!(mgr.active_job_count(), 1);
        assert!(mgr.is_converting());

        mgr.complete_job(id3);
        assert_eq!(mgr.active_job_count(), 0);
        assert!(!mgr.is_converting());
    }

    #[test]
    fn test_job_manager_failed_job() {
        let mut mgr = JobManager::new();
        let (id, _) = mgr.register_job(Path::new("corrupt.mp4"), None, 10, None, 101, None);

        mgr.fail_job(id, "Corrupted video file".to_string());

        assert!(!mgr.is_converting());
        let job = mgr.get_job(id).unwrap();
        assert_eq!(job.state, JobState::Failed);
        assert_eq!(job.error.as_deref(), Some("Corrupted video file"));
    }

    #[test]
    fn test_job_manager_stale_job() {
        let mut mgr = JobManager::new();
        let (id, _) = mgr.register_job(Path::new("old.mp4"), None, 9, None, 101, None);

        mgr.mark_stale(id);

        assert!(!mgr.is_converting());
        let job = mgr.get_job(id).unwrap();
        assert_eq!(job.state, JobState::Stale);
    }

    #[test]
    fn test_job_manager_multiple_generations() {
        let mut mgr = JobManager::new();

        let (id1, _) = mgr.register_job(Path::new("vid1.mp4"), None, 10, None, 1, None);
        let (id2, _) = mgr.register_job(Path::new("vid2.mp4"), None, 15, None, 2, None);

        assert_eq!(mgr.get_job(id1).unwrap().latest_generation(), 10);
        assert_eq!(mgr.get_job(id2).unwrap().latest_generation(), 15);
    }

    #[test]
    fn test_job_manager_duplicate_source_dedup() {
        let mut mgr = JobManager::new();

        // Client 1 requests video.mp4 (gen 10)
        let (id1, is_new1) = mgr.register_job(Path::new("video.mp4"), None, 10, None, 101, None);
        assert!(is_new1);
        assert_eq!(id1, 1);

        // Client 2 requests SAME video.mp4 while job 1 is in-flight (gen 11)
        let (id2, is_new2) = mgr.register_job(Path::new("video.mp4"), None, 11, None, 102, None);
        assert!(!is_new2, "Duplicate request must not trigger new worker");
        assert_eq!(id2, id1, "Must attach to existing in-flight job");

        // Job has both subscribers and preserves respective request metadata
        let job = mgr.get_job(id1).unwrap();
        assert_eq!(job.subscribers.len(), 2);
        assert_eq!(job.subscribers[0].request_id, 101);
        assert_eq!(job.subscribers[0].generation, 10);
        assert_eq!(job.subscribers[1].request_id, 102);
        assert_eq!(job.subscribers[1].generation, 11);
        assert_eq!(job.latest_generation(), 11);
    }

    #[test]
    fn test_job_manager_content_hash_dedup_different_paths() {
        let mut mgr = JobManager::new();
        let hash = "deadbeef12345678";

        // Request 1: from /home/user/video.mp4
        let (id1, is_new1) = mgr.register_job(
            Path::new("/home/user/video.mp4"),
            Some(hash),
            10,
            Some("DP-1".into()),
            201,
            None,
        );
        assert!(is_new1);

        // Request 2: from /media/usb/backup_video.mp4 with IDENTICAL content hash
        let (id2, is_new2) = mgr.register_job(
            Path::new("/media/usb/backup_video.mp4"),
            Some(hash),
            12, // newer generation
            Some("DP-1".into()),
            202,
            None,
        );
        assert!(
            !is_new2,
            "Same content hash must be deduplicated across different paths"
        );
        assert_eq!(id1, id2);

        let job = mgr.get_job(id1).unwrap();
        assert_eq!(job.subscribers.len(), 2);
        assert_eq!(job.subscribers[0].generation, 10);
        assert_eq!(job.subscribers[1].generation, 12);
        assert_eq!(
            job.latest_generation(),
            12,
            "Latest generation must advance to newest request"
        );
    }

    #[test]
    fn test_job_manager_concurrent_same_hash_different_outputs() {
        let mut mgr = JobManager::new();
        let hash = "multi_output_hash_12345";

        // Request A: DP-1, gen 10
        let (id1, is_new1) = mgr.register_job(
            Path::new("shared_video.mp4"),
            Some(hash),
            10,
            Some("DP-1".into()),
            1001,
            Some(ClientId(10)),
        );
        assert!(is_new1, "First request must spawn job");

        // Request B: DP-2, gen 11 (concurrently arrives while Job 1 is converting)
        let (id2, is_new2) = mgr.register_job(
            Path::new("shared_video.mp4"),
            Some(hash),
            11,
            Some("DP-2".into()),
            1002,
            Some(ClientId(11)),
        );
        assert!(
            !is_new2,
            "Second request must deduplicate and NOT spawn a second worker"
        );
        assert_eq!(id1, id2, "Must share the exact same conversion job ID");

        let job = mgr.get_job(id1).unwrap();
        assert_eq!(job.subscribers.len(), 2);

        // Crucial verification: Subscriber A maintains DP-1 / gen 10
        let sub_a = &job.subscribers[0];
        assert_eq!(sub_a.request_id, 1001);
        assert_eq!(sub_a.client_id, Some(ClientId(10)));
        assert_eq!(sub_a.generation, 10);
        assert_eq!(sub_a.target_output.as_deref(), Some("DP-1"));

        // Crucial verification: Subscriber B maintains DP-2 / gen 11 (NOT overwriting sub A)
        let sub_b = &job.subscribers[1];
        assert_eq!(sub_b.request_id, 1002);
        assert_eq!(sub_b.client_id, Some(ClientId(11)));
        assert_eq!(sub_b.generation, 11);
        assert_eq!(sub_b.target_output.as_deref(), Some("DP-2"));

        // Summarized target output string should show both outputs
        let info = job.to_info();
        assert_eq!(info.generation, 11);
        assert_eq!(info.target_output.as_deref(), Some("DP-1, DP-2"));
    }

    #[test]
    fn test_job_manager_conversion_failure_followed_by_retry() {
        let mut mgr = JobManager::new();
        let hash = "fail_then_retry_hash";

        // 1. Initial attempt fails
        let (id1, is_new1) =
            mgr.register_job(Path::new("faulty.mp4"), Some(hash), 1, None, 10, None);
        assert!(is_new1);
        mgr.fail_job(id1, "Transcode crash".to_string());
        assert!(!mgr.is_converting());

        // 2. Retry attempt for same hash -> must spawn new job (is_new == true)
        let (id2, is_new2) =
            mgr.register_job(Path::new("faulty.mp4"), Some(hash), 2, None, 11, None);
        assert!(is_new2, "Retrying failed job must spawn a fresh worker");
        assert_ne!(id1, id2);
        assert_eq!(id2, 2);
        assert!(mgr.is_converting());
    }

    #[test]
    fn test_job_manager_concurrent_duplicate_requests() {
        use std::sync::{Arc, Mutex};
        use std::thread;

        let mgr = Arc::new(Mutex::new(JobManager::new()));
        let hash = "concurrent_hash_test";

        let mut handles = Vec::new();
        for i in 1..=5 {
            let mgr_clone = mgr.clone();
            handles.push(thread::spawn(move || {
                let mut guard = mgr_clone.lock().unwrap();
                guard.register_job(
                    Path::new("same_file.mp4"),
                    Some(hash),
                    10 + i,
                    Some(format!("DP-{i}")),
                    100 + i,
                    Some(ClientId(i)),
                )
            }));
        }

        let mut new_count = 0;
        let mut job_ids = Vec::new();
        for h in handles {
            let (job_id, is_new) = h.join().unwrap();
            if is_new {
                new_count += 1;
            }
            job_ids.push(job_id);
        }

        // Exactly one thread must get is_new == true
        assert_eq!(
            new_count, 1,
            "Only one worker should be spawned for concurrent identical requests"
        );
        // All threads must point to the same job_id
        for id in &job_ids {
            assert_eq!(*id, job_ids[0]);
        }

        let guard = mgr.lock().unwrap();
        let job = guard.get_job(job_ids[0]).unwrap();
        assert_eq!(job.subscribers.len(), 5);
        assert_eq!(
            job.latest_generation(),
            15,
            "Latest generation (10 + 5) must be preserved"
        );

        // Verify all 5 outputs are present
        let outputs: Vec<Option<String>> = job
            .subscribers
            .iter()
            .map(|s| s.target_output.clone())
            .collect();
        for i in 1..=5 {
            assert!(outputs.contains(&Some(format!("DP-{i}"))));
        }
    }

    #[test]
    fn test_job_manager_cancel_superseded_jobs() {
        let mut mgr = JobManager::new();

        // 1. Register job #1 for DP-1 with generation 10
        let (id1, is_new1) = mgr.register_job(
            Path::new("video1.mp4"),
            Some("hash1"),
            10,
            Some("DP-1".to_string()),
            1001,
            Some(ClientId(1)),
        );
        assert!(is_new1);
        let cancel_token1 = mgr.cancel_token(id1).unwrap();
        assert!(!cancel_token1.load(std::sync::atomic::Ordering::SeqCst));

        // 2. Register job #2 for DP-2 with generation 10
        let (id2, is_new2) = mgr.register_job(
            Path::new("video2.mp4"),
            Some("hash2"),
            10,
            Some("DP-2".to_string()),
            1002,
            Some(ClientId(2)),
        );
        assert!(is_new2);
        let cancel_token2 = mgr.cancel_token(id2).unwrap();
        assert!(!cancel_token2.load(std::sync::atomic::Ordering::SeqCst));

        // 3. User immediately requests new video on DP-1 with generation 11
        // Calling cancel_superseded_jobs for DP-1 with gen 11 must cancel job #1, but keep job #2 running
        let cancelled = mgr.cancel_superseded_jobs(Some("DP-1"), 11);
        assert_eq!(cancelled, vec![id1]);
        assert!(cancel_token1.load(std::sync::atomic::Ordering::SeqCst));
        assert!(!cancel_token2.load(std::sync::atomic::Ordering::SeqCst));

        let job1 = mgr.get_job(id1).unwrap();
        assert_eq!(job1.state, JobState::Cancelled);

        let job2 = mgr.get_job(id2).unwrap();
        assert_eq!(job2.state, JobState::Queued);

        // 4. Global request with generation 12 supersedes all remaining in-flight jobs
        let cancelled_global = mgr.cancel_superseded_jobs(None, 12);
        assert_eq!(cancelled_global, vec![id2]);
        assert!(cancel_token2.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn test_job_manager_dedup_then_cancel_preserves_inflight() {
        let mut mgr = JobManager::new();

        // 1. Initial request for DP-1 with gen 5
        let (id1, is_new1) = mgr.register_job(
            Path::new("video1.mp4"),
            Some("hash1"),
            5,
            Some("DP-1".to_string()),
            1001,
            Some(ClientId(1)),
        );
        assert!(is_new1);

        // 2. Second request arrives for same video on DP-1 with higher gen 6
        // Registering first deduplicates and attaches subscriber to id1
        let (id2, is_new2) = mgr.register_job(
            Path::new("video1.mp4"),
            Some("hash1"),
            6,
            Some("DP-1".to_string()),
            1002,
            Some(ClientId(2)),
        );
        assert!(!is_new2);
        assert_eq!(id1, id2);

        // 3. Cancelling superseded jobs for DP-1 while excluding id2 must NOT cancel id1
        let cancelled = mgr.cancel_superseded_jobs_except(Some("DP-1"), 6, Some(id2));
        assert!(
            cancelled.is_empty(),
            "Deduplicated active job must not be cancelled"
        );

        let job = mgr.get_job(id1).unwrap();
        assert_ne!(job.state, JobState::Cancelled);
        assert_eq!(job.subscribers.len(), 2);
    }
}
